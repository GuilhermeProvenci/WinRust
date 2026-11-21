//! Read-only, bounded translation catalogs. Missing or invalid entries fall
//! back to the source text; catalogs never contain executable code.
use std::{collections::HashMap, io::Read, path::Path, sync::{OnceLock, atomic::{AtomicU64, Ordering::Relaxed}}};

const LIMIT: u64 = 2 * 1024 * 1024;
static RUSSIAN: OnceLock<HashMap<String, String>> = OnceLock::new();
static RU_CHECK: AtomicU64 = AtomicU64::new(0);
static PORTUGUESE: OnceLock<HashMap<String, String>> = OnceLock::new();
static JAPANESE: OnceLock<HashMap<String, String>> = OnceLock::new();
static PT_CHECK: AtomicU64 = AtomicU64::new(0);
static JA_CHECK: AtomicU64 = AtomicU64::new(0);

fn parse(bytes: &[u8], locale: &str) -> Option<HashMap<String, String>> {
    if bytes.len() as u64 > LIMIT || !bytes.starts_with(b"WRLANG\x01\0") { return None; }
    let mut remaining = &bytes[8..];
    fn number(data: &mut &[u8]) -> Option<usize> {
        let (value, rest) = data.split_first_chunk::<4>()?;
        *data = rest;
        Some(u32::from_le_bytes(*value) as usize)
    }
    fn string(data: &mut &[u8], len: usize) -> Option<String> {
        if len > 32_768 || len > data.len() { return None; }
        let (value, rest) = data.split_at(len);
        *data = rest;
        let value = std::str::from_utf8(value).ok()?;
        if value.contains('\0') { return None; }
        Some(value.to_string())
    }
    let locale_len = number(&mut remaining)?;
    if string(&mut remaining, locale_len)? != locale { return None; }
    let count = number(&mut remaining)?;
    if count > 20_000 { return None; }
    let mut map = HashMap::with_capacity(count);
    for _ in 0..count {
        let key_len = number(&mut remaining)?;
        let key = string(&mut remaining, key_len)?;
        let text_len = number(&mut remaining)?;
        let text = string(&mut remaining, text_len)?;
        if map.insert(key, text).is_some() { return None; }
    }
    if !remaining.is_empty() { return None; }
    Some(map)
}

fn read(path: &Path, locale: &str) -> Option<HashMap<String, String>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(LIMIT + 1).read_to_end(&mut bytes).ok()?;
    parse(&bytes, locale)
}
pub fn valid_file(path: &Path, locale: &str) -> bool { read(path, locale).is_some() }
pub fn valid_bytes(bytes: &[u8], locale: &str) -> bool { parse(bytes, locale).is_some() }

fn load(locale: &str) -> Option<HashMap<String, String>> {
    // An adjacent catalog lets the installer display translations before the
    // pack is installed; applications read the installed machine-wide copy.
    let name = format!("{locale}.wrlang");
    let beside = std::env::current_exe().ok()?.parent()?.join("locales").join(&name);
    let shared = std::env::var_os("ProgramData").map(|d| std::path::PathBuf::from(d).join("WinRust").join("locales").join(name));
    read(&beside, locale).or_else(|| shared.and_then(|p| read(&p, locale)))
}

fn cached(locale: &str) -> Option<&'static HashMap<String, String>> {
    let (cache, checked) = match locale { "ru" => (&RUSSIAN, &RU_CHECK), "pt-BR" => (&PORTUGUESE, &PT_CHECK), "ja" => (&JAPANESE, &JA_CHECK), _ => return None };
    if cache.get().is_none() {
        let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }.saturating_add(1);
        let old = checked.load(Relaxed);
        if (old == 0 || now.saturating_sub(old) >= 1000) && checked.compare_exchange(old, now, Relaxed, Relaxed).is_ok() {
            if let Some(value) = load(locale) { let _ = cache.set(value); }
        }
    }
    cache.get()
}
pub fn lookup(locale: &str, key: &str) -> Option<&'static str> { cached(locale)?.get(key).map(String::as_str) }
pub fn lookup_current(key: &str) -> Option<&'static str> { lookup(super::language(), key) }
pub const fn context_hash(text: &str) -> u64 { let bytes = text.as_bytes(); let mut i = 0; let mut hash = 14695981039346656037u64; while i < bytes.len() { hash = (hash ^ bytes[i] as u64).wrapping_mul(1099511628211); i += 1; } hash }
pub fn lookup_context(english: &str, context: u64) -> Option<&'static str> {
    if super::language() == "ru" { lookup("ru", &format!("{english}\u{1f}{context:016x}")) } else { lookup_current(english) }
}
pub fn available(locale: &str) -> bool { cached(locale).is_some() }
pub fn retry_missing() { RU_CHECK.store(0, Relaxed); PT_CHECK.store(0, Relaxed); JA_CHECK.store(0, Relaxed); }

/// Parse Rust-style placeholders and escaped braces. Implicit arguments are
/// numbered in encounter order, so incompatible translations are rejected.
fn parts(template: &str) -> Option<Vec<(bool, String)>> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut chars = template.chars().peekable();
    let mut implicit = 0usize;
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') { chars.next(); text.push('{'); continue; }
            if !text.is_empty() { out.push((false, std::mem::take(&mut text))); }
            let mut field = String::new();
            let mut closed = false;
            for ch in chars.by_ref() {
                if ch == '}' { closed = true; break; }
                if ch == '{' { return None; }
                field.push(ch);
            }
            if !closed { return None; }
            let (name, spec) = field.split_once(':').map_or((field.as_str(), ""), |(n, s)| (n, s));
            let key = if name.is_empty() { let n = implicit; implicit += 1; format!("{n}:{spec}") } else { format!("{name}:{spec}") };
            out.push((true, key));
        } else if c == '}' {
            if chars.peek() == Some(&'}') { chars.next(); text.push('}'); } else { return None; }
        } else { text.push(c); }
    }
    if !text.is_empty() { out.push((false, text)); }
    Some(out)
}

pub fn compatible(source: &str, translation: &str) -> bool {
    let keys = |s: &str| -> Option<Vec<String>> {
        let mut k: Vec<_> = parts(s)?.into_iter().filter_map(|(field, value)| field.then_some(value)).collect();
        k.sort(); Some(k)
    };
    matches!((keys(source), keys(translation)), (Some(a), Some(b)) if a == b)
}

pub fn render(template: &str, values: &[(&str, String)]) -> Option<String> {
    let mut out = String::new();
    for (field, value) in parts(template)? {
        if field { out.push_str(&values.iter().find(|(key, _)| *key == value)?.1); }
        else { out.push_str(&value); }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_schema_locale_and_bounds() {
        let mut bytes = b"WRLANG\x01\0".to_vec();
        for value in [b"pt-BR".as_slice(), b"Hello", b"Ola"] {
            bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
            bytes.extend_from_slice(value);
            if value == b"pt-BR" { bytes.extend_from_slice(&1u32.to_le_bytes()); }
        }
        assert_eq!(parse(&bytes, "pt-BR").unwrap().get("Hello").unwrap(), "Ola");
        assert!(parse(&bytes, "ja").is_none());
        bytes.pop(); assert!(parse(&bytes, "pt-BR").is_none());
        assert!(parse(&vec![b' '; LIMIT as usize + 1], "pt-BR").is_none());
    }
    #[test]
    fn translated_templates_preserve_arguments_and_escapes() {
        assert!(compatible("{name}: {:.0}% / {}", "{} / {name}: {:.0}%") == false);
        assert!(compatible("{name}: {n:.0} / {}", "{} / {name}: {n:.0}"));
        assert!(!compatible("{x}", "{y}"));
        assert!(!compatible("{x}", "{x"));
        let vals = [("name:", "Ana".to_string()), ("0:", "42".to_string())];
        assert_eq!(render("{{{name}}} = {}", &vals).as_deref(), Some("{Ana} = 42"));
    }
}

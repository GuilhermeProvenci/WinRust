//! Code shared by the WinRust programs: the language switch (`t!`, `tf!`, `en!`), the NTFS MFT reader, Win+D.
//! Use it with `#[macro_use] extern crate winrust_common;` at the crate root.
//!
//! English by default; Russian when %ProgramData%\WinRust\lang.txt contains "ru" (the installer's EN/RU button writes it).
//! Programs read the file once at start; the one that flips the switch updates its own texts at once.
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU8, AtomicU64, Ordering::Relaxed},
};
extern crate self as winrust_common;
pub use winrust_i18n_macros::{localized_format, localized_format_context};

pub mod desktop;
pub mod journal;
pub mod launch;
pub mod locales;
pub mod mem;
pub mod mft;
pub mod proc;
pub mod reg;
pub mod settings;
pub mod style;
pub mod screens;

/// "Show in folder": opens `path`'s folder with the file selected in the installed rust-explorer, else in Windows Explorer.
pub fn reveal(path: &str) {
    use std::{os::windows::process::CommandExt, process::Command};
    let rx = std::env::var_os("ProgramFiles").map(|pf| PathBuf::from(pf).join(r"rust-explorer\rust-explorer.exe")).filter(|p| p.is_file());
    let _ = match rx {
        Some(rx) => Command::new(rx).raw_arg(format!("/select,\"{path}\"")).spawn(),
        None => Command::new("explorer.exe").raw_arg(format!("/select,\"{path}\"")).spawn(),
    };
}

static LANG: AtomicU8 = AtomicU8::new(0); // 0 = unread, 1 = English, 2 = Russian, 3 = pt-BR, 4 = ja
static LANG_CHECK: AtomicU64 = AtomicU64::new(0);

fn file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("WinRust").join("lang.txt"))
}
fn legacy_file() -> Option<PathBuf> { std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("WinRust").join("lang.txt")) }

pub fn language() -> &'static str {
    let mut value = LANG.load(Relaxed);
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }.saturating_add(1);
    let checked = LANG_CHECK.load(Relaxed);
    if (value == 0 || now.saturating_sub(checked) >= 1000) && LANG_CHECK.compare_exchange(checked, now, Relaxed, Relaxed).is_ok() {
        let saved = file().and_then(|f| std::fs::read_to_string(f).ok())
            .or_else(|| legacy_file().and_then(|f| std::fs::read_to_string(f).ok()));
        value = match saved.as_deref().map(str::trim) {
            Some(s) if s.eq_ignore_ascii_case("ru") && locales::available("ru") => 2,
            Some(s) if s.eq_ignore_ascii_case("pt-BR") || s.eq_ignore_ascii_case("pt") => 3,
            Some(s) if s.eq_ignore_ascii_case("ja") => 4,
            Some(_) => 1,
            None => match (unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff) {
                0x16 if locales::available("pt-BR") => 3,
                0x11 if locales::available("ja") => 4,
                0x19 if locales::available("ru") => 2,
                _ => 1,
            },
        };
        LANG.store(value, Relaxed);
    }
    match value { 2 => "ru", 3 => "pt-BR", 4 => "ja", _ => "en" }
}

/// The current language at run time. In code that has unit tests use `en!()` instead: it is always Russian under `cargo test`.
pub fn english() -> bool {
    language() == "en"
}

/// Switches this process and writes the choice for the other programs (they pick it up when they start).
pub fn set_english(en: bool) {
    let _ = try_set_english(en);
}
pub fn try_set_english(en: bool) -> std::io::Result<()> {
    set_language(if en { "en" } else { "pt-BR" })
}
pub fn set_language(lang: &str) -> std::io::Result<()> {
    let v = match lang { "en" => 1, "ru" => 2, "pt-BR" => 3, "ja" => 4,
        _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "unsupported language")) };
    if v >= 2 && !locales::available(lang) { return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "language pack is not installed")); }
    let f = file().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not available"))?;
    write_language(&f, lang)?;
    if let Some(legacy) = legacy_file() { let _ = write_language(&legacy, lang); }
    LANG.store(v, Relaxed);
    LANG_CHECK.store(unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }.saturating_add(1), Relaxed);
    Ok(())
}
fn write_language(f: &std::path::Path, lang: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(f.parent().unwrap())?;
    let tmp = f.with_file_name(format!("lang-{}.tmp", std::process::id()));
    std::fs::write(&tmp, lang)?;
    use std::os::windows::ffi::OsStrExt;
    let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = f.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(),
        windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH) } == 0 {
        let error = std::io::Error::last_os_error();
        let _ = std::fs::remove_file(tmp);
        return Err(error);
    }
    Ok(())
}

/// `en!()`: true for English. `cfg!(test)` is checked in the calling crate, so its unit tests keep their Russian expectations.
#[macro_export]
macro_rules! en {
    () => {
        !cfg!(test) && $crate::english()
    };
}

/// `t!("русский", "english")`: a text in the current language.
#[macro_export]
macro_rules! t {
    ($ru:literal, $en:expr) => {{
        const CONTEXT: u64 = $crate::locales::context_hash($ru);
        if cfg!(test) { $ru } else { $crate::locales::lookup_context($en, CONTEXT).unwrap_or($en) }
    }};
    ($ru:expr, $en:expr) => {{
        if cfg!(test) { $ru } else { $crate::locales::lookup_context($en, $crate::locales::context_hash($ru)).unwrap_or($en) }
    }};
}

/// `tf!("русский {}", "english {}", args...)`: format! in the current language (both templates are checked at compile time).
#[macro_export]
macro_rules! tf {
    ($ru:literal, $en:literal $(, $a:expr)* $(,)?) => {
        if cfg!(test) { format!($ru $(, $a)*) } else { $crate::localized_format_context!($ru, $en $(, $a)*) }
    };
}

#[macro_export]
macro_rules! pt {
    ($ru:literal, $en:expr, $pt:expr) => {{
        const CONTEXT: u64 = $crate::locales::context_hash($ru);
        if cfg!(test) { $ru } else { $crate::locales::lookup_context($en, CONTEXT).unwrap_or($en) }
    }};
    ($ru:expr, $en:expr, $pt:expr) => {{
        if cfg!(test) { $ru } else { $crate::locales::lookup_context($en, $crate::locales::context_hash($ru)).unwrap_or($en) }
    }};
}
#[macro_export]
macro_rules! ptf {
    ($ru:literal, $en:literal, $pt:literal $(, $a:expr)* $(,)?) => {
        if cfg!(test) { format!($ru $(, $a)*) } else { $crate::localized_format_context!($ru, $en $(, $a)*) }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn tests_run_in_russian() {
        assert_eq!(t!("да", "yes"), "да");
        assert_eq!(tf!("{} шт", "{} pcs", 3), "3 шт");
    }
}

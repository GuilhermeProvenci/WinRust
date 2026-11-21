//! Optional data-only catalogs distributed next to the installer.
use std::{fs, io::Read, path::{Path, PathBuf}};

pub const PACKS: [(&str, &str); 3] = [("ru", "Русский"), ("pt-BR", "Português (Brasil)"), ("ja", "日本語")];

pub fn source_dir() -> Option<PathBuf> { Some(std::env::current_exe().ok()?.parent()?.join("locales")) }
pub fn installed_dir() -> Option<PathBuf> { Some(PathBuf::from(std::env::var_os("ProgramData")?).join("WinRust/locales")) }

fn filename(locale: &str) -> Option<String> {
    PACKS.iter().any(|(id, _)| *id == locale).then(|| format!("{locale}.wrlang"))
}
pub fn valid_in(dir: &Path, locale: &str) -> bool {
    filename(locale).is_some_and(|name| winrust_common::locales::valid_file(&dir.join(name), locale))
}
fn read_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Only install selected catalogs. Existing unselected catalogs are preserved.
/// Every selected source is validated before any file is replaced.
pub fn install(source: &Path, dest: &Path, selected: &[&str]) -> Result<(), String> {
    let mut todo = Vec::new();
    for &locale in selected {
        let name = filename(locale).ok_or_else(|| format!("{}: {locale}", winrust_common::t!("Неподдерживаемый язык", "Unsupported language")))?;
        let src = source.join(&name);
        if !valid_in(source, locale) {
            if !src.exists() && valid_in(dest, locale) { continue; }
            return Err(format!("{}: {locale}", winrust_common::t!("Языковой пакет отсутствует или повреждён", "Missing or invalid language pack")));
        }
        let bytes = read_bounded(&src).map_err(|e| format!("{}: {e}", src.display()))?;
        if !winrust_common::locales::valid_bytes(&bytes, locale) { return Err(format!("{}: {locale}", winrust_common::t!("Повреждённый языковой пакет", "Invalid language pack"))); }
        todo.push((bytes, dest.join(name)));
    }
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for (bytes, target) in todo {
        if read_bounded(&target).is_ok_and(|old| old == bytes) { continue; }
        let tmp = target.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        use std::os::windows::ffi::OsStrExt;
        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe { windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH) };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            let _ = fs::remove_file(tmp);
            return Err(format!("{}: {error}", target.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pack(locale: &str, value: &str) -> Vec<u8> {
        let mut out = b"WRLANG\x01\0".to_vec();
        fn put(out: &mut Vec<u8>, s: &str) { out.extend_from_slice(&(s.len() as u32).to_le_bytes()); out.extend_from_slice(s.as_bytes()); }
        put(&mut out, locale); out.extend_from_slice(&1u32.to_le_bytes()); put(&mut out, "Hello"); put(&mut out, value); out
    }
    #[test]
    fn installs_only_valid_selected_packs_and_preserves_existing_files() {
        let base = std::env::temp_dir().join(format!("winrust-install-pack-{}", std::process::id()));
        let src = base.join("src"); let dest = base.join("dest");
        fs::create_dir_all(&src).unwrap(); fs::create_dir_all(&dest).unwrap();
        fs::write(src.join("pt-BR.wrlang"), pack("pt-BR", "Olá")).unwrap();
        fs::write(dest.join("ja.wrlang"), pack("ja", "こんにちは")).unwrap();
        install(&src, &dest, &["pt-BR"]).unwrap();
        assert!(valid_in(&dest, "pt-BR")); assert!(valid_in(&dest, "ja"));
        let before = fs::read(dest.join("pt-BR.wrlang")).unwrap();
        fs::write(src.join("pt-BR.wrlang"), b"invalid").unwrap();
        assert!(install(&src, &dest, &["pt-BR", "unknown"]).is_err());
        assert_eq!(fs::read(dest.join("pt-BR.wrlang")).unwrap(), before);
        fs::remove_dir_all(base).unwrap();
    }
}

//! The fork updater only accepts signed releases published by its owner.
use crate::*;
use crate::update_manifest::{sha256_hex, ver, verify, Manifest};
use std::{io::Write as _, os::windows::fs::MetadataExt};

const BASE: &str = "https://github.com/GuilhermeProvenci/WinRust/releases";
const KEY: &str = include_str!("../fork-update-public-key.txt");
pub(crate) static AVAILABLE: Mutex<Option<String>> = Mutex::new(None);

fn fetch(url: &str, max_size: usize) -> Result<Vec<u8>, String> {
    let mut system = vec![0u16; 32768];
    let length = unsafe { windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    if length == 0 || length >= system.len() { return Err("Cannot locate the Windows system directory".into()); }
    system.truncate(length);
    let curl = PathBuf::from(String::from_utf16(&system).map_err(|_| "Invalid system directory")?).join("curl.exe");
    let result = Command::new(curl).args(["--disable", "--fail", "--silent", "--show-error", "--location", "--max-redirs", "5", "--proto", "=https", "--proto-redir", "=https", "--connect-timeout", "10", "--max-time", "120", "--max-filesize", &max_size.to_string(), url]).env_remove("CURL_CA_BUNDLE").env_remove("SSL_CERT_FILE").env_remove("SSL_CERT_DIR").creation_flags(0x0800_0000).output().map_err(|e| e.to_string())?;
    if !result.status.success() { return Err("No accessible signed release for this fork".into()); }
    if result.stdout.len() > max_size { return Err("Release file exceeds size limit".into()); }
    Ok(result.stdout)
}
fn manifest() -> Result<Manifest, String> {
    let text = fetch(&format!("{BASE}/latest/download/manifest.txt"), 64 * 1024)?;
    let sig = fetch(&format!("{BASE}/latest/download/manifest.sig"), 4096)?;
    verify(&text, std::str::from_utf8(&sig).map_err(|_| "invalid signature encoding")?, KEY)
}
fn protected_root() -> Result<PathBuf, String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let mut buf = vec![0u16; 32768]; let mut len = (buf.len() * 2) as u32;
    let key = wide(r"SOFTWARE\Microsoft\Windows\CurrentVersion");
    let name = wide("ProgramFilesDir");
    let status = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut len) };
    if status != 0 || len < 2 { return Err("Cannot find trusted Program Files directory".into()); }
    buf.truncate((len as usize / 2).saturating_sub(1));
    let base = PathBuf::from(String::from_utf16(&buf).map_err(|_| "Invalid Program Files path")?);
    ensure_directory(&base, false)?;
    let root = base.join("WinRustFork"); ensure_directory(&root, true)?;
    Ok(root)
}
fn ensure_directory(path: &Path, create: bool) -> Result<(), String> {
    if create {
        match fs::create_dir(path) { Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}, Err(e) => return Err(e.to_string()) }
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 { return Err(format!("Unsafe update directory: {}", path.display())); }
    Ok(())
}
fn saved_version() -> Result<Option<String>, String> {
    let path = protected_root()?.join("installed-version.txt");
    match fs::read_to_string(path) {
        Ok(s) => { let value = s.trim().to_string(); if ver(&value).is_none() { Err("Invalid installed fork version record".into()) } else { Ok(Some(value)) } },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
fn installed_version() -> Result<String, String> {
    let built = FORK_VERSION.trim();
    match saved_version()? {
        Some(s) if ver(&s) > ver(built) => Ok(s),
        Some(_) => Ok(built.to_string()),
        None if ready().any(|i| (PLUGINS[i].installed)()) => Err("Run the current fork installer once to register its version before online updates".into()),
        None => Ok(built.to_string()),
    }
}
pub(crate) fn reject_older_installer() -> Result<(), String> {
    if saved_version()?.is_some_and(|saved| ver(&saved) > ver(FORK_VERSION.trim())) { return Err("This installer is older than the installed fork".into()); }
    Ok(())
}
pub(crate) fn mark_installed() -> Result<(), String> {
    let d = protected_root()?;
    if saved_version()?.is_some_and(|saved| ver(&saved) > ver(FORK_VERSION.trim())) { return Err("An older installer cannot replace the recorded version".into()); }
    let tmp = d.join(format!("installed-version-{}.tmp", std::process::id()));
    fs::write(&tmp, FORK_VERSION.trim()).map_err(|e| e.to_string())?;
    let from = wide(&tmp.to_string_lossy()); let to = wide(&d.join("installed-version.txt").to_string_lossy());
    let ok = unsafe { windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(), windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH) };
    if ok == 0 { let e = std::io::Error::last_os_error(); let _ = fs::remove_file(tmp); return Err(e.to_string()); }
    Ok(())
}
pub(crate) fn check() -> Result<Option<String>, String> {
    let result = (|| { let m = manifest()?; Ok((ver(&m.version) > ver(&installed_version()?)).then_some(m.version)) })();
    *AVAILABLE.lock().unwrap() = result.as_ref().ok().cloned().flatten();
    result
}
/// Download each signed asset to a new administrator-only Program Files folder,
/// verify all bytes and open the installer. It never installs components itself.
pub(crate) fn download_and_open() -> Result<String, String> {
    let m = manifest()?;
    if ver(&m.version) <= ver(&installed_version()?) { return Err("The release is not newer than the installed version".into()); }
    let mut files = Vec::new();
    for (expected, name) in &m.files {
        let bytes = fetch(&format!("{BASE}/download/{}/{}", m.version, name), 64 * 1024 * 1024)?;
        if sha256_hex(&bytes) != *expected { return Err(format!("{name}: hash mismatch")); }
        if name.ends_with(".wrlang") {
            let locale = name.trim_end_matches(".wrlang");
            if !i18n::locales::valid_bytes(&bytes, locale) { return Err(format!("{name}: invalid language pack")); }
        }
        files.push((name.clone(), bytes));
    }
    let parent = protected_root()?.join("updates");
    ensure_directory(&parent, true)?;
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let dir = parent.join(format!("update-{}-{nonce}", std::process::id()));
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let locale_dir = dir.join("locales"); fs::create_dir(&locale_dir).map_err(|e| e.to_string())?;
    for (name, bytes) in files {
        let dest = if name == "WinRust.exe" { dir.join(&name) } else { locale_dir.join(&name) };
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(dest).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?; file.sync_all().map_err(|e| e.to_string())?;
    }
    Command::new(dir.join("WinRust.exe")).spawn().map_err(|e| e.to_string())?;
    Ok(format!("Verified fork installer {} opened", m.version))
}

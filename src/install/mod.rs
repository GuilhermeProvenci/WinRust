//! Install logic, shared by the window and the command line so both do exactly the same thing:
//! process/registry helpers, the embedded payload, and one file per plugin with its install/uninstall pair.
use crate::*;
mod explorer;
mod journal;
mod search;
mod session;
mod taskmgr;
mod tools;
mod topbar;
mod content_compare;
pub(crate) use {explorer::*, journal::*, search::*, session::*, taskmgr::*, tools::*, topbar::*};

pub(crate) fn out(prog: &str, args: &[&str]) -> Result<String, String> {
    // reg.exe: HKCU means the signed-in user's, not the elevated account's (see session.rs)
    let owned: Vec<String> = if prog == "reg" { i18n::reg::user_args(args) } else { args.iter().map(|a| a.to_string()).collect() };
    let o = Command::new(prog).args(&owned).creation_flags(0x0800_0000).output().map_err(|e| format!("{prog}: {e}"))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        Err(format!("{prog} {}: {}{}", owned.join(" "), String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim()))
    }
}

pub(crate) fn run(prog: &str, args: &[&str]) -> Result<(), String> {
    out(prog, args).map(|_| ())
}

// Non-fatal failures of one install: shown as a count in the status line, spelled out in install.log.
pub(crate) static WARN: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub(crate) fn soft(what: &str, r: Result<(), String>) {
    if let Err(e) = r {
        WARN.lock().unwrap().push(format!("{what}: {e}"));
    }
}

pub(crate) fn program_data() -> PathBuf {
    PathBuf::from(env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into()))
}

/// Start Menu shortcut: search and the bar's Start panel list Start Menu shortcuts only.
pub(crate) fn start_lnk(name: &str) -> PathBuf {
    program_data().join(format!(r"Microsoft\Windows\Start Menu\Programs\{name}.lnk"))
}

pub(crate) fn shortcut(lnk: &Path, target: &Path, args: &str) -> Result<(), String> {
    run("powershell", &["-NoProfile", "-Command", &format!("$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath='{}'; $s.Arguments='{args}'; $s.Save()", lnk.display(), target.display())])
}

pub(crate) fn log(s: &str) {
    let d = program_data().join("plugin-launcher");
    let _ = fs::create_dir_all(&d);
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(d.join("install.log")) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = writeln!(f, "[{t}] {s}"); // shortcut: unix seconds, no date formatting; upgrade if the log gets read by humans a lot
    }
}

/// Copies over an exe that was just killed: taskkill returns before Windows (or an antivirus scan) releases the file, so retry ~6 s.
pub(crate) fn put(src: &Path, dst: &Path) -> Result<(), String> {
    let mut r = fs::copy(src, dst).map(|_| ());
    for _ in 0..30 {
        if !matches!(&r, Err(e) if e.raw_os_error() == Some(32) || e.raw_os_error() == Some(5)) {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(200));
        r = fs::copy(src, dst).map(|_| ());
    }
    r.map_err(|e| format!("{}: {e}", dst.display()))
}

pub(crate) fn reg_dword(key: &str, name: &str) -> Option<u32> {
    i18n::reg::dword(&out("reg", &["query", key, "/v", name]).ok()?)
}

/// A REG_SZ (or any string-like) value as text.
pub(crate) fn reg_str(key: &str, name: &str) -> Option<String> {
    i18n::reg::value(&out("reg", &["query", key, "/v", name]).ok()?).map(|(_, d)| d.to_string())
}

include!(concat!(env!("OUT_DIR"), "/payload.rs")); // PAYLOAD: the exes listed in payload.txt, embedded by build.rs

/// Update without rebuilding the launcher: an exe in `payload\` next to this one (or two levels up, the cargo
/// target\release layout) wins over the embedded copy. Trust level = the launcher exe's own folder. Two levels up only from
/// target\release: from C:\Program Files\WinRust it would be C:\payload, which any user may create.
pub(crate) fn disk_payload() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let d = exe.parent()?;
    [Some(d.join("payload")), d.ends_with(r"target\release").then(|| d.join(r"..\..\payload"))].into_iter().flatten().find(|p| p.is_dir())
}

/// Compare all executables owned by an installed component against the same
/// source with_payload will use. Unknown or unreadable means "update".
pub(crate) fn component_current(name: &str) -> bool {
    let files: Vec<(&str, PathBuf)> = match name {
        "rust-search" => vec![("rs.exe", prog_dir().join("rs.exe")), ("rs-launcher.exe", prog_dir().join("rs-launcher.exe"))],
        "rtaskmgr-win32" => vec![("Task manager.exe", tm_dir().join("rtaskmgr-win32.exe")), ("rs-du.exe", tm_dir().join("rs-du.exe")), ("debloat.exe", tm_dir().join("debloat.exe"))],
        "topbar" => vec![("topbar.exe", bar_dir().join("topbar.exe")), ("reshell.exe", bar_dir().join("reshell.exe")), ("rs-du.exe", tm_dir().join("rs-du.exe"))],
        "rust-explorer" => vec![("rust-explorer.exe", rx_dir().join("rust-explorer.exe")), ("rs-shell.exe", rx_dir().join("rs-shell.exe"))],
        "wiztree" => vec![("wiztree.exe", wz_dir().join("wiztree.exe"))],
        "photoview" => vec![("photoview.exe", pv_dir().join("photoview.exe"))],
        "rust-settings" => vec![("rust-settings.exe", st_dir().join("rust-settings.exe"))],
        // These actions alter system configuration, not merely installed files.
        _ => return false,
    };
    let disk = disk_payload();
    files.into_iter().all(|(name, installed)| {
        let Some(embedded) = PAYLOAD.iter().find(|(n, _)| *n == name).map(|(_, bytes)| *bytes) else { return false };
        let source = disk.as_ref().map(|p| p.join(name)).filter(|p| p.is_file());
        let expected = match source { Some(path) => match fs::read(path) { Ok(bytes) => bytes, Err(_) => return false }, None => embedded.to_vec() };
        content_compare::matches_file(&installed, &expected)
    })
}

/// Unpacks the `need`ed bundled plugins, runs `f` on them, deletes them. The folder is under Program Files (admin-only writable),
/// not %TEMP%: a user-level process must not be able to swap an exe that is about to be installed as a SYSTEM task.
pub(crate) fn with_payload<T>(need: &[&str], f: impl FnOnce(&Path) -> Result<T, String>) -> Result<T, String> {
    let d = payload_dir()?;
    let disk = disk_payload();
    let unpack = |n: &&str| -> Result<(), String> {
        let bytes = PAYLOAD.iter().find(|(m, _)| m == n).ok_or(ptf!("нет {n} в сборке", "{n} is missing from the build", "{n} está faltando na compilação"))?.1;
        match disk.as_ref().map(|p| p.join(n)).filter(|p| p.is_file()) {
            Some(src) => {
                log(&ptf!("{n}: из {}", "{n}: from {}", "{n}: de {}", src.display()));
                fs::copy(src, d.join(n)).map(|_| ())
            }
            None => fs::write(d.join(n), bytes),
        }
        .map_err(|e| ptf!("распаковка {n}: {e}", "unpacking {n}: {e}", "desempacotando {n}: {e}"))
    };
    let r = need.iter().try_for_each(unpack).and_then(|_| f(&d));
    if let Err(e) = fs::remove_dir_all(&d) {
        log(&format!("cannot remove {}: {e}", d.display())); // left behind: the next call uses another name, so it is harmless
    }
    r
}

/// A fresh folder under Program Files for one `with_payload` call. A new name every time (process id + a counter): a folder
/// the previous call removed can still be "pending delete" while something (a scanner, a process started from it) holds a
/// handle, and creating that name again then fails with "access denied". Creation is retried a few times, and every failure
/// goes to the log: the first one seen in the field (rust-search and rtaskmgr-win32 at the start of an update, then fine)
/// left no trace.
fn payload_dir() -> Result<PathBuf, String> {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let mut last = String::new();
    for attempt in 0..5 {
        let d = program_files().join(format!("plugin-launcher-tmp-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        match fs::create_dir_all(&d) {
            Ok(()) => return Ok(d),
            Err(e) => {
                last = format!("{}: {e}", d.display());
                log(&format!("cannot create {last} (attempt {})", attempt + 1));
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        }
    }
    Err(last)
}

pub(crate) fn program_files() -> PathBuf {
    PathBuf::from(env::var("ProgramFiles").unwrap_or(r"C:\Program Files".into()))
}

// shortcut: Windows makes at most one restore point per 24 h and it needs System Protection on; failures are ignored.
pub(crate) fn checkpoint() -> Result<(), String> {
    run("powershell", &["-NoProfile", "-Command", "Checkpoint-Computer -Description 'Plugin Launcher' -RestorePointType MODIFY_SETTINGS"])
}

// Shared by the window and the command line, so both do exactly the same thing.
pub(crate) fn install_plugin(i: usize) -> Result<String, String> {
    let p = &PLUGINS[i];
    let (inst, uninst) = p.actions.ok_or(pt!("установка недоступна", "installation is not available", "a instalação não está disponível"))?;
    init().inspect_err(|e| log(&ptf!("{}: ОШИБКА {e}", "{}: ERROR {e}", "{}: ERRO {e}", p.name)))?; // before anything is written: no user, no install
    let was = (p.installed)(); // already installed = an update: a failure must not remove the working copy
    WARN.lock().unwrap().clear();
    soft(pt!("точка восстановления не создана", "restore point was not created", "ponto de restauração não foi criado"), checkpoint());
    if let Err(e) = inst() {
        log(&ptf!("{}: ОШИБКА {e}", "{}: ERROR {e}", "{}: ERRO {e}", p.name));
        if was {
            return Err(e);
        }
        let rollback = match uninst() {
            Ok(()) => pt!("автоматический откат завершён", "automatic rollback completed", "reversão automática concluída").to_string(),
            Err(u) => ptf!("ошибка автоматического отката: {u}", "automatic rollback failed: {u}", "falha na reversão automática: {u}"),
        };
        log(&format!("{}: {rollback}", p.name));
        return Err(format!("{e} ({rollback})"));
    }
    let w = WARN.lock().unwrap().clone();
    log(&format!("{}: {} {}", p.name, if was { pt!("обновлён", "updated", "atualizado") } else { pt!("установлен", "installed", "instalado") }, w.join(" | ")));
    let note = if w.is_empty() { String::new() } else { ptf!(" Предупреждений: {} (см. %ProgramData%\\plugin-launcher\\install.log).", " Warnings: {} (see %ProgramData%\\plugin-launcher\\install.log).", " Avisos: {} (consulte %ProgramData%\\plugin-launcher\\install.log).", w.len()) };
    Ok(format!("{} {}.{note}", p.name, if was { pt!("обновлён", "updated", "atualizado") } else { pt!("установлен", "installed", "instalado") }))
}

pub(crate) fn uninstall_plugin(i: usize) -> Result<String, String> {
    let p = &PLUGINS[i];
    let (_, uninst) = p.actions.ok_or(pt!("удаление недоступно", "uninstall is not available", "a desinstalação não está disponível"))?;
    WARN.lock().unwrap().clear();
    init_soft();
    uninst().inspect_err(|e| log(&ptf!("{}: ОШИБКА удаления {e}", "{}: uninstall ERROR {e}", "{}: ERRO de desinstalação {e}", p.name)))?;
    let w = WARN.lock().unwrap().clone();
    log(&format!("{}: {} {}", p.name, pt!("удалён", "removed", "removido"), w.join(" | ")));
    let note = if w.is_empty() { String::new() } else { ptf!(" Предупреждений: {} (см. %ProgramData%\\plugin-launcher\\install.log).", " Warnings: {} (see %ProgramData%\\plugin-launcher\\install.log).", " Avisos: {} (consulte %ProgramData%\\plugin-launcher\\install.log).", w.len()) };
    Ok(ptf!("{} удалён.{note}", "{} removed.{note}", "{} removido.{note}", p.name))
}

pub(crate) fn rollback() -> Result<String, String> {
    WARN.lock().unwrap().clear();
    init_soft();
    // reverse install order: rust-explorer (which switched Explorer off) goes before topbar, else the session is left with no panel and no shell
    let errs: Vec<String> = PLUGINS.iter().rev().filter(|p| (p.installed)()).filter_map(|p| p.actions.and_then(|a| a.1().err().map(|e| format!("{}: {e}", p.name)))).collect();
    let warnings = WARN.lock().unwrap().join(" | ");
    let note = if warnings.is_empty() { "" } else {
        log(&format!("rollback: {warnings}"));
        pt!(" Есть предупреждения (см. %ProgramData%\\plugin-launcher\\install.log).", " See warnings in %ProgramData%\\plugin-launcher\\install.log.", " Veja os avisos em %ProgramData%\\plugin-launcher\\install.log.")
    };
    if errs.is_empty() {
        Ok(ptf!("Все плагины удалены.{note}", "All plugins removed.{note}", "Todos os plug-ins removidos.{note}"))
    } else {
        Err(format!("{}{note}", errs.join("; ")))
    }
}

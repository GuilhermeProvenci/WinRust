//! topbar: kbhook — the thread that owns the low-level keyboard hook (split out of main.rs; shares its items through `super::*`).
//!
//! Windows removes a low-level hook without a word when its callback does not answer within `LowLevelHooksTimeout`, and no
//! call tells us it happened: Win+R, Win+X, Start and Alt+Tab then stay dead with no explorer to fall back on. The cause
//! of the drops reported upstream is not known. Measured here (12 logical CPUs, all busy, hook thread at below-normal /
//! normal / time-critical priority): every injected key went through the hook in under 1 ms, so CPU starvation of the
//! hook thread is not it, and raising its priority does nothing. So this module does three things:
//!  * sets the hook again every `REHOOK_MS` and after a failed attempt (one dropped hook must not last the whole session);
//!  * watches for a dropped hook: raw input is not subject to the hook timeout, so when it sees physical keys and the hook
//!    sees none for `WATCH_MS`, the hook is gone: it is set again at once;
//!  * writes `%APPDATA%\topbar-hook.log`: every drop it caught and every callback slower than `SLOW_MS`, with the time and
//!    how long Windows had been up, so the real cause can be read off the next occurrence. Key codes are never logged.
use super::*;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed as R};
use windows_sys::Win32::UI::Input::{GetRawInputData, RAWINPUTDEVICE, RAWINPUTHEADER, RID_HEADER, RIDEV_INPUTSINK, RegisterRawInputDevices};

/// How often the hook is set again. Cheap (one call); long enough that a swap is never in the way of typing.
const REHOOK_MS: u32 = 30_000;
/// How often the watch compares what raw input and the hook saw.
const WATCH_MS: u32 = 2_000;
/// Fewer physical key events than this in a watch period prove nothing (one key can fall between two periods).
const WATCH_MIN: u32 = 3;
/// A callback this slow is logged: Windows allows 300 ms by default (`LowLevelHooksTimeout`, 1 s at most).
const SLOW_MS: u128 = 100;
const LOG_MAX: u64 = 64 * 1024;

static HOOK_N: AtomicU32 = AtomicU32::new(0); // physical (not injected) key events the hook callback received
static RAW_N: AtomicU32 = AtomicU32::new(0); // physical key events raw input delivered

/// The hook callback, timed. `inner` is the real work (main.rs `kb_hook_inner`).
pub(crate) unsafe fn timed(n: i32, w: WPARAM, l: LPARAM, inner: unsafe fn(i32, WPARAM, LPARAM) -> LRESULT) -> LRESULT {
    if n >= 0 && (*(l as *const KBDLLHOOKSTRUCT)).flags & LLKHF_INJECTED == 0 {
        HOOK_N.fetch_add(1, R);
    }
    let t = std::time::Instant::now();
    let r = inner(n, w, l);
    let took = t.elapsed();
    if took.as_millis() >= SLOW_MS {
        log(&format!("slow callback: {} ms (Windows drops a hook past ~300 ms)", took.as_millis()));
    }
    r
}

/// One line in the log (with the local time and Windows' uptime). Past `LOG_MAX` the old file is kept as `.old`.
fn log(line: &str) {
    log_to(&appdata_file("topbar-hook.log"), line);
}

/// The hook is gone when raw input saw enough physical keys in a watch period and the hook saw none.
fn hook_lost(raw_seen: u32, hook_seen: u32) -> bool {
    raw_seen >= WATCH_MIN && hook_seen == 0
}

fn log_to(path: &std::path::Path, line: &str) {
    use std::io::Write;
    if std::fs::metadata(path).is_ok_and(|m| m.len() > LOG_MAX) {
        let _ = std::fs::rename(path, path.with_extension("log.old"));
    }
    let t = unsafe { let mut t: SYSTEMTIME = zeroed(); GetLocalTime(&mut t); t };
    let up = unsafe { GetTickCount64() } / 1000;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{:04}-{:02}-{:02} {:02}:{:02}:{:02} (Windows up {}h{:02}m) {}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, up / 3600, up / 60 % 60, line);
    }
}

unsafe fn install() -> HHOOK {
    let h = SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_hook), GetModuleHandleW(null()), 0);
    if h.is_null() {
        log(&format!("SetWindowsHookExW failed (error {})", GetLastError()));
    }
    h
}

/// Sets the hook again and drops the old one. The new one goes in first, so there is no moment without a hook; and since
/// callbacks only run inside `GetMessageW` on this thread, both can never be in the middle of the same key. Keeps `old`
/// when the new one cannot be set.
unsafe fn refresh(old: HHOOK) -> HHOOK {
    let new = install();
    if new.is_null() {
        return old;
    }
    if !old.is_null() {
        UnhookWindowsHookEx(old);
    }
    new
}

/// A window for raw input to go to (a message-only one: nothing to see), registered for keyboards, also in the background.
unsafe fn raw_window() {
    let win = CreateWindowExW(0, wide("STATIC").as_ptr(), null(), 0, 0, 0, 0, 0, HWND_MESSAGE, null_mut(), GetModuleHandleW(null()), null());
    let dev = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 6, dwFlags: RIDEV_INPUTSINK, hwndTarget: win };
    if win.is_null() || RegisterRawInputDevices(&dev, 1, size_of::<RAWINPUTDEVICE>() as u32) == 0 {
        log(&format!("raw input watch not available (error {}): a dropped hook will only be fixed by the timer", GetLastError()));
    }
}

/// A WM_INPUT from a physical keyboard counts (keys we inject ourselves have no device).
unsafe fn count_raw(l: LPARAM) {
    let mut h: RAWINPUTHEADER = zeroed();
    let mut size = size_of::<RAWINPUTHEADER>() as u32;
    if GetRawInputData(l as _, RID_HEADER, &mut h as *mut _ as _, &mut size, size_of::<RAWINPUTHEADER>() as u32) != u32::MAX && !h.hDevice.is_null() {
        RAW_N.fetch_add(1, R);
    }
}

/// Starts the hook thread (the primary bar only: one hook per session).
pub(crate) fn spawn() {
    thread::spawn(|| unsafe {
        let mut hook = install();
        let mut set_at = std::time::Instant::now();
        log(&format!("hook {} (bar pid {})", if hook.is_null() { "NOT set" } else { "set" }, std::process::id()));
        raw_window();
        let rehook = SetTimer(null_mut(), 0, REHOOK_MS, None);
        let watch = SetTimer(null_mut(), 0, WATCH_MS, None);
        let (mut last_raw, mut last_hook) = (0u32, 0u32);
        let mut m: MSG = zeroed();
        while GetMessageW(&mut m, null_mut(), 0, 0) > 0 {
            if m.message == WM_INPUT {
                count_raw(m.lParam);
            } else if m.message == WM_TIMER && m.wParam == rehook {
                hook = refresh(hook);
                set_at = std::time::Instant::now();
            } else if m.message == WM_TIMER && m.wParam == watch {
                let (raw, hk) = (RAW_N.load(R), HOOK_N.load(R));
                let (dr, dh) = (raw.wrapping_sub(last_raw), hk.wrapping_sub(last_hook));
                (last_raw, last_hook) = (raw, hk);
                if hook_lost(dr, dh) {
                    log(&format!("hook lost: raw input saw {dr} physical key events in {} s, the hook none (hook set {} s ago); set again", WATCH_MS / 1000, set_at.elapsed().as_secs()));
                    hook = refresh(hook);
                    set_at = std::time::Instant::now();
                }
            }
            TranslateMessage(&m);
            DispatchMessageW(&m);
        }
        if !hook.is_null() {
            UnhookWindowsHookEx(hook);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_is_lost_only_when_raw_input_saw_keys_and_it_saw_none() {
        assert!(hook_lost(5, 0));
        assert!(!hook_lost(5, 1)); // it answered at least once: alive
        assert!(!hook_lost(0, 0)); // nobody typed
        assert!(!hook_lost(WATCH_MIN - 1, 0)); // too few keys to say
    }

    #[test]
    fn the_log_gets_timestamped_lines_and_rotates_when_big() {
        let dir = std::env::temp_dir().join(format!("kbhook-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("topbar-hook.log");
        log_to(&path, "first");
        log_to(&path, "second");
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().all(|l| l.contains("(Windows up ")) && text.contains("first") && text.ends_with("second\n"));
        std::fs::write(&path, vec![b'x'; LOG_MAX as usize + 1]).unwrap();
        log_to(&path, "after rotation");
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1); // a fresh file
        assert!(dir.join("topbar-hook.log.old").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    unsafe fn quick(_: i32, _: WPARAM, _: LPARAM) -> LRESULT { 7 }

    #[test]
    fn only_physical_keys_are_counted_and_the_answer_is_passed_on() {
        unsafe {
            let mut k: KBDLLHOOKSTRUCT = zeroed();
            let before = HOOK_N.load(R);
            k.flags = LLKHF_INJECTED;
            assert_eq!(timed(0, 0, &k as *const _ as LPARAM, quick), 7);
            assert_eq!(HOOK_N.load(R), before, "injected keys are not counted");
            k.flags = 0;
            assert_eq!(timed(0, 0, &k as *const _ as LPARAM, quick), 7);
            assert_eq!(HOOK_N.load(R), before + 1);
        }
    }
}

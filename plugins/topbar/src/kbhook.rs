//! topbar: kbhook — the thread that owns the low-level keyboard hook (split out of main.rs; shares its items through `super::*`).
//!
//! Windows removes a low-level hook without a word when its thread does not answer within `LowLevelHooksTimeout` (a busy
//! boot is enough), and there is no call that tells us. Win+R, Win+X, Start and Alt+Tab then stay dead with no explorer to
//! fall back on. Three things keep that from happening or make it short-lived:
//!  * the thread runs at the highest priority, so it answers even when the machine is loaded (the cause);
//!  * the callback itself stays small: anything slow is posted to the bar's own thread (see `RUN_BOX`);
//!  * the hook is set again every `REHOOK_MS` (the cure for the one that still gets dropped), and again after a failed
//!    first attempt, so a hook that could not be set during a bad moment of the boot is not lost for the whole session.
use super::*;
use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};

/// How often the hook is set again. Cheap (one call); long enough that a swap is never in the way of typing.
const REHOOK_MS: u32 = 30_000;

unsafe fn install() -> HHOOK {
    SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_hook), GetModuleHandleW(null()), 0)
}

/// Sets the hook again and drops the old one. The new one goes in first, so there is no moment without a hook; and since
/// callbacks only run inside `GetMessageW` on this thread, both can never be in the middle of the same key. Keeps `old`
/// when the new one cannot be set. Not while an Alt+Tab is on screen: its Alt release must reach the same hook.
unsafe fn refresh(old: HHOOK) -> HHOOK {
    if !old.is_null() && SWITCHING.load(Relaxed) {
        return old;
    }
    let new = install();
    if new.is_null() {
        return old;
    }
    if !old.is_null() {
        UnhookWindowsHookEx(old);
    }
    new
}

/// Starts the hook thread (the primary bar only: one hook per session).
pub(crate) fn spawn() {
    thread::spawn(|| unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        let mut hook = install();
        let timer = SetTimer(null_mut(), 0, REHOOK_MS, None);
        let mut m: MSG = zeroed();
        while GetMessageW(&mut m, null_mut(), 0, 0) > 0 {
            if m.message == WM_TIMER && m.wParam == timer {
                hook = refresh(hook);
            }
        }
        if !hook.is_null() {
            UnhookWindowsHookEx(hook);
        }
    });
}

//! topbar: kbhook — the thread that owns the low-level keyboard hook (split out of main.rs; shares its items through `super::*`).
//!
//! Windows removes a low-level hook without a word when its callback does not answer within `LowLevelHooksTimeout`, and no
//! call tells us it happened: Win+R, Win+X, Start and Alt+Tab then stay dead with no explorer to fall back on. The cause
//! of the drops reported upstream is not known. Measured here (12 logical CPUs, all busy, hook thread at below-normal /
//! normal / time-critical priority): every injected key went through the hook in under 1 ms, so CPU starvation of the
//! hook thread is not it, and raising its priority does nothing. What is left is to set the hook again from time to time
//! (what other hook users do) and again after a failed first attempt, so one dropped hook, or one that could not be set
//! during a bad moment of the boot, does not last for the whole session.
use super::*;

/// How often the hook is set again. Cheap (one call); long enough that a swap is never in the way of typing.
const REHOOK_MS: u32 = 30_000;

unsafe fn install() -> HHOOK {
    SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_hook), GetModuleHandleW(null()), 0)
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

/// Starts the hook thread (the primary bar only: one hook per session).
pub(crate) fn spawn() {
    thread::spawn(|| unsafe {
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

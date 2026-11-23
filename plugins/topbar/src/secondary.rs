//! Per-monitor visibility for Explorer's secondary taskbars.
//! Explorer's auto-hide setting is global, so this keeps each existing secondary
//! taskbar on its monitor while the primary WinRust bar owns its own setting.
#![allow(unsafe_op_in_unsafe_fn)]
use super::*;

#[derive(Clone)]
pub struct Screen { pub name: String, hwnd: HWND, monitor: HMONITOR, rect: RECT }
impl Screen { pub fn hwnd(&self) -> HWND { self.hwnd } }
thread_local! { static HIDE_SETTINGS: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new()); }

pub unsafe fn screens() -> Vec<Screen> {
    let class = wide("Shell_SecondaryTrayWnd");
    let mut hwnd = null_mut();
    let mut out = Vec::new();
    loop {
        hwnd = FindWindowExW(null_mut(), hwnd, class.as_ptr(), null());
        if hwnd.is_null() { break; }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFOEXW = zeroed();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO) == 0 { continue; }
        let name = wstr_of(&info.szDevice);
        if name.is_empty() { continue; }
        out.push(Screen { name, hwnd, monitor, rect: info.monitorInfo.rcMonitor });
    }
    out
}

fn settings_path(name: &str) -> PathBuf {
    let safe: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    appdata_file(&format!("topbar-monitor-{safe}-autohide.txt"))
}

pub fn autohide(name: &str) -> bool {
    std::fs::read_to_string(settings_path(name)).is_ok_and(|v| v.trim() == "1")
}

pub fn toggle(name: &str) {
    let next = !autohide(name);
    if std::fs::write(settings_path(name), if next { "1" } else { "0" }).is_ok() {
        HIDE_SETTINGS.with_borrow_mut(|settings| { settings.insert(name.to_string(), next); });
    }
}

fn pointer_over_bar(pt: POINT, monitor: RECT, height: i32, already_shown: bool) -> bool {
    pt.x >= monitor.left && pt.x < monitor.right && pt.y >= monitor.top && pt.y < monitor.bottom &&
        (pt.y >= monitor.bottom - 2 || (already_shown && pt.y >= monitor.bottom - height))
}

fn should_show(hide_automatically: bool, fullscreen: bool, pointer_over: bool) -> bool {
    (!hide_automatically && !fullscreen) || pointer_over
}

unsafe fn covers_monitor(monitor: HMONITOR, screen: RECT) -> bool {
    let fg = GetForegroundWindow();
    if fg.is_null() || IsIconic(fg) != 0 || IsWindowVisible(fg) == 0 || MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST) != monitor { return false; }
    let mut class = [0u16; 64];
    let n = GetClassNameW(fg, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
    if matches!(String::from_utf16_lossy(&class[..n]).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "RustExplorer") { return false; }
    if ordinary_maximized(IsZoomed(fg) != 0, GetWindowLongPtrW(fg, GWL_STYLE) as u32) { return false; }
    let mut r: RECT = zeroed();
    if GetWindowRect(fg, &mut r) == 0 { return false; }
    r.left <= screen.left && r.top <= screen.top && r.right >= screen.right && r.bottom >= screen.bottom
}

pub unsafe fn update() {
    if foreground_protected_game() {
        for screen in screens() { if IsWindowVisible(screen.hwnd) != 0 { ShowWindow(screen.hwnd, SW_HIDE); } }
        return;
    }
    let mut cursor = POINT { x: 0, y: 0 };
    GetCursorPos(&mut cursor);
    for s in screens() {
        let mut r: RECT = zeroed();
        if GetWindowRect(s.hwnd, &mut r) == 0 { continue; }
        let height = r.bottom - r.top;
        let width = r.right - r.left;
        // Do not reposition an unusual vertical taskbar; Explorer keeps control.
        if height <= 0 || width <= height { continue; }
        let visible = IsWindowVisible(s.hwnd) != 0 && r.top < s.rect.bottom - 2;
        let over = pointer_over_bar(cursor, s.rect, height, visible);
        let show = should_show(autohide(&s.name), covers_monitor(s.monitor, s.rect), over);
        if show {
            let x = s.rect.left;
            let y = s.rect.bottom - height;
            let w = s.rect.right - s.rect.left;
            if !visible || r.left != x || r.top != y || width != w {
                SetWindowPos(s.hwnd, HWND_TOPMOST, x, y, w, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
        } else if IsWindowVisible(s.hwnd) != 0 {
            ShowWindow(s.hwnd, SW_HIDE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_detection_stays_on_the_correct_monitor() {
        let m = RECT { left: 1920, top: 323, right: 3280, bottom: 1091 };
        assert!(pointer_over_bar(POINT { x: 2000, y: 1090 }, m, 40, false));
        assert!(!pointer_over_bar(POINT { x: 100, y: 1090 }, m, 40, false));
        assert!(!pointer_over_bar(POINT { x: 2000, y: 1060 }, m, 40, false));
        assert!(pointer_over_bar(POINT { x: 2000, y: 1060 }, m, 40, true));
    }
    #[test]
    fn fixed_and_auto_hidden_modes_are_independent() {
        assert!(should_show(false, false, false));
        assert!(!should_show(true, false, false));
        assert!(should_show(true, false, true));
        assert!(!should_show(false, true, false));
        assert!(should_show(false, true, true));
    }
}

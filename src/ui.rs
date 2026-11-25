//! The window: drawing (GDI+ shapes, GDI text), layout, dialogs and the message loop handler.
use crate::*;

// ---------- drawing ----------
//
// One back buffer per paint: GDI+ for the anti-aliased shapes, then GDI (ClearType) for text on top.

pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// COLORREF (0x00BBGGRR) + alpha -> GDI+ ARGB.
pub(crate) const fn argb(c: u32, a: u8) -> u32 {
    (a as u32) << 24 | (c & 255) << 16 | (c >> 8 & 255) << 8 | (c >> 16 & 255)
}

pub(crate) fn lerp(a: u32, b: u32, t: f32) -> u32 {
    let m = |sh: u32| (((a >> sh & 255) as f32) * (1.0 - t) + ((b >> sh & 255) as f32) * t) as u32;
    m(0) | m(8) << 8 | m(16) << 16
}

pub(crate) type Box4 = [f32; 4]; // left, top, right, bottom in logical px

pub(crate) struct Ui {
    dc: HDC,
    g: *mut GpGraphics,
    s: f32,
}
impl Ui {
    fn px(&self, v: f32) -> i32 {
        (v * self.s).round() as i32
    }
    unsafe fn shape(&self, b: Box4, rad: f32, color: u32) {
        let (s, mut path) = (self.s, null_mut::<GpPath>());
        let (x, y, w, h) = (b[0] * s, b[1] * s, (b[2] - b[0]) * s, (b[3] - b[1]) * s);
        let d = (rad * 2.0 * s).min(h).min(w).max(0.01);
        GdipCreatePath(FillModeAlternate, &mut path);
        GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
        GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
        GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
        GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
        GdipClosePathFigure(path);
        let mut br = null_mut::<GpSolidFill>();
        GdipCreateSolidFill(color, &mut br);
        GdipFillPath(self.g, br as *mut GpBrush, path);
        GdipDeleteBrush(br as *mut GpBrush);
        GdipDeletePath(path);
    }
    /// Rounded box with a 1 px outline (`line` == `fill` for none).
    unsafe fn round(&self, b: Box4, rad: f32, fill: u32, line: u32) {
        if line != fill {
            self.shape(b, rad, argb(line, 255));
            let i = 1.0 / self.s;
            self.shape([b[0] + i, b[1] + i, b[2] - i, b[3] - i], rad - i, argb(fill, 255));
        } else {
            self.shape(b, rad, argb(fill, 255));
        }
    }
    unsafe fn font(&self, size: f32, weight: i32, face: &str) -> HFONT {
        CreateFontW(-self.px(size), 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, wide(face).as_ptr())
    }
    unsafe fn text_in(&self, s: &str, b: Box4, size: f32, weight: i32, color: u32, flags: u32, face: &str) {
        let f = self.font(size, weight, face);
        let of = SelectObject(self.dc, f);
        SetTextColor(self.dc, color);
        SetBkMode(self.dc, TRANSPARENT as i32);
        let mut r = RECT { left: self.px(b[0]), top: self.px(b[1]), right: self.px(b[2]), bottom: self.px(b[3]) };
        DrawTextW(self.dc, wide(s).as_ptr(), -1, &mut r, flags | DT_NOPREFIX);
        SelectObject(self.dc, of);
        DeleteObject(f);
    }
    unsafe fn text(&self, s: &str, b: Box4, size: f32, weight: i32, color: u32, flags: u32) {
        self.text_in(s, b, size, weight, color, flags, winrust_common::style::sans_face());
    }
    /// Width of a single line of text, logical px.
    unsafe fn width(&self, s: &str, size: f32, weight: i32) -> f32 {
        let f = self.font(size, weight, winrust_common::style::sans_face());
        let of = SelectObject(self.dc, f);
        let mut r: RECT = zeroed();
        DrawTextW(self.dc, wide(s).as_ptr(), -1, &mut r, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(self.dc, of);
        DeleteObject(f);
        r.right as f32 / self.s
    }
}

// layout, logical px
pub(crate) const WIN_W: f32 = 600.0;
pub(crate) const M: f32 = 24.0; // outer margin
pub(crate) const N_READY: usize = {
    let (mut n, mut i) = (0, 0);
    while i < PLUGINS.len() {
        if PLUGINS[i].actions.is_some() && PLUGINS[i].bulk {
            n += 1;
        }
        i += 1;
    }
    n
};
pub(crate) const LIST_TOP: f32 = 136.0;
pub(crate) const ROW_H: f32 = 62.0;
pub(crate) const N_EXTRA: usize = {
    let (mut n, mut i) = (0, 0);
    while i < PLUGINS.len() { if PLUGINS[i].actions.is_some() && !PLUGINS[i].bulk { n += 1; } i += 1; }
    n
};
pub(crate) const EXTRA_GAP: f32 = 30.0;
pub(crate) const LIST_H: f32 = (N_READY + N_EXTRA) as f32 * ROW_H + 12.0 + EXTRA_GAP;
pub(crate) const MAIN_TOP: f32 = LIST_TOP + LIST_H + 20.0;
pub(crate) const MAIN_H: f32 = 54.0;
pub(crate) const FOOT_H: f32 = 72.0;
pub(crate) const WIN_H: f32 = MAIN_TOP + MAIN_H + 16.0 + FOOT_H;
pub(crate) const BTN_W: f32 = 136.0;
pub(crate) const BTN_H: f32 = 36.0;
// clickable things
pub(crate) const ID_MAIN: usize = 0;
pub(crate) const ID_ROLLBACK: usize = 1;
pub(crate) const ID_OK: usize = 2;
pub(crate) const ID_CANCEL: usize = 3;
pub(crate) const ID_SWITCH: usize = 4;
pub(crate) const ID_GEAR: usize = 5;
pub(crate) const ID_LANG: usize = 6; // EN / RU switch
pub(crate) const ID_LP_EN: usize = 8;
pub(crate) const ID_LP_JA: usize = 11;
pub(crate) const ID_PACK_PT: usize = 12;
pub(crate) const ID_PACK_JA: usize = 14;
pub(crate) const ID_PACK_APPLY: usize = 15;
pub(crate) const ID_PACK_CLOSE: usize = 16;
pub(crate) const ID_SHELL: usize = 7; // settings panel: replace Explorer with rust-explorer
pub(crate) const ID_ALL: usize = 17;
pub(crate) const ID_UPDATE: usize = 18;
pub(crate) const ID_CHECK: usize = 100; // + plugin index: the row's checkbox
pub(crate) const STARTING: usize = usize::MAX - 1; // JOB before install_all picks its first component

/// Installable components, in install order (topbar comes before rust-explorer, which needs it).
pub(crate) fn ready() -> impl Iterator<Item = usize> {
    (0..PLUGINS.len()).filter(|&i| PLUGINS[i].actions.is_some() && PLUGINS[i].bulk)
}

pub(crate) fn extras() -> impl Iterator<Item = usize> {
    (0..PLUGINS.len()).filter(|&i| PLUGINS[i].actions.is_some() && !PLUGINS[i].bulk)
}
pub(crate) fn rows() -> impl Iterator<Item = usize> { ready().chain(extras()) }
/// Items marked for installation, in install order.
pub(crate) fn chosen() -> impl Iterator<Item = usize> { rows().filter(|&i| SELECTED[i].load(Relaxed)) }
fn planned() -> (Vec<usize>, Vec<usize>) {
    plan(&rows().collect::<Vec<_>>(), |i| INSTALLED[i].load(Relaxed), |i| SELECTED[i].load(Relaxed))
}
fn toggle_component(i: usize) {
    if i >= PLUGINS.len() || PLUGINS[i].actions.is_none() { return; }
    let next = !SELECTED[i].load(Relaxed);
    SELECTED[i].store(next, Relaxed);
    if let Some((other, wanted)) = linked(PLUGINS[i].name, next) {
        if let Some(j) = PLUGINS.iter().position(|p| p.name == other) { SELECTED[j].store(wanted, Relaxed); }
    }
}
fn update_box(w: f32) -> Box4 { [w - M - 246.0, 92.0, w - M, 107.0] }
fn all_box(w: f32) -> Box4 { [w - M - 150.0, LIST_TOP - 28.0, w - M, LIST_TOP - 3.0] }


/// Checkbox at the left end of row `k`, and the larger area that toggles it.
pub(crate) fn check_box(k: usize, w: f32) -> (Box4, Box4) {
    let r = row_box(k, w);
    ([r[0] + 12.0, r[1] + 22.0, r[0] + 30.0, r[1] + 40.0], [r[0] + 4.0, r[1] + 8.0, r[0] + 38.0, r[3] - 8.0])
}

pub(crate) fn row_box(k: usize, w: f32) -> Box4 {
    let t = LIST_TOP + 6.0 + k as f32 * ROW_H + if k >= N_READY { EXTRA_GAP } else { 0.0 };
    [M + 6.0, t, w - M - 6.0, t + ROW_H]
}

/// Settings gear at the right end of the row of the plugin that has settings (topbar): (plugin index, box).
pub(crate) fn gear_box(w: f32) -> Option<(usize, Box4)> {
    let (k, i) = ready().enumerate().find(|&(_, i)| PLUGINS[i].settings)?;
    let r = row_box(k, w);
    let t = r[1] + (ROW_H - 32.0) / 2.0;
    Some((i, [r[2] - 14.0 - 32.0, t, r[2] - 14.0, t + 32.0]))
}

pub(crate) fn main_box(w: f32) -> Box4 {
    [M, MAIN_TOP, w - M, MAIN_TOP + MAIN_H]
}

/// On/off switch inside the settings panel row.
pub(crate) fn switch_box(w: f32) -> Box4 {
    let (d, _, _) = dialog_boxes(w);
    [d[2] - 24.0 - 16.0 - 44.0, d[1] + 78.0 + 23.0, d[2] - 24.0 - 16.0, d[1] + 78.0 + 47.0]
}

/// Second switch row of the settings panel: which shell to install.
pub(crate) fn shell_switch_box(w: f32) -> Box4 {
    let b = switch_box(w);
    [b[0], b[1] + 80.0, b[2], b[3] + 80.0]
}

/// Index of rust-explorer in PLUGINS.
fn explorer_idx() -> Option<usize> {
    PLUGINS.iter().position(|p| p.name == crate::selection::RUST_EXPLORER)
}

/// The English / Portuguese switch in the top right corner of the header.
pub(crate) fn lang_box(w: f32) -> Box4 {
    [w - M - 96.0, 18.0, w - M, 18.0 + 30.0]
}

fn locale_choice_box(w: f32, i: usize) -> Box4 {
    let (d, _, _) = dialog_boxes(w);
    let x = d[0] + 24.0 + (i % 2) as f32 * 238.0;
    let y = d[1] + 92.0 + (i / 2) as f32 * 34.0;
    [x, y, x + 226.0, y + 30.0]
}
fn pack_choice_box(w: f32, i: usize) -> Box4 {
    let (d, _, _) = dialog_boxes(w);
    let y = d[1] + 207.0 + i as f32 * 32.0;
    [d[0] + 24.0, y, d[2] - 24.0, y + 29.0]
}

pub(crate) fn rollback_box(w: f32) -> Box4 {
    let t = WIN_H - FOOT_H + (FOOT_H - BTN_H) / 2.0;
    [w - M - 150.0, t, w - M, t + BTN_H]
}

/// Dialog panel and its (cancel, ok) buttons.
pub(crate) fn dialog_boxes(w: f32) -> (Box4, Box4, Box4) {
    let (dw, dh) = (528.0, 392.0);
    let (l, t) = ((w - dw) / 2.0, (WIN_H - dh) / 2.0);
    let (bt, r) = (t + dh - 24.0 - BTN_H, l + dw - 24.0);
    ([l, t, l + dw, t + dh], [r - BTN_W - 10.0 - 120.0, bt, r - BTN_W - 10.0, bt + BTN_H], [r - BTN_W, bt, r, bt + BTN_H])
}

/// Fit the complete installer inside the usable desktop at the current DPI.
pub(crate) unsafe fn window_scale(dc: HDC) -> f32 {
    let dpi = GetDeviceCaps(dc, LOGPIXELSX as i32) as f32 / 96.0;
    let mut work: RECT = zeroed();
    if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut _ as *mut _, 0) == 0 { return dpi; }
    let max_h = ((work.bottom - work.top - GetSystemMetrics(SM_CYCAPTION) - 24).max(200) as f32) / WIN_H;
    let max_w = ((work.right - work.left - 24).max(200) as f32) / WIN_W;
    dpi.min(max_h).min(max_w).max(0.25)
}

/// A button: `kind` 0 = primary (accent), 1 = secondary (outline), 2 = danger (outline, red text), 3 = disabled.
pub(crate) unsafe fn button(ui: &Ui, b: Box4, label: &str, kind: u8, hot: bool) {
    let (fill, line, fg) = match kind {
        0 => (ACCENT, ACCENT, rgb(255, 255, 255)),
        1 => (PANEL, BORDER, TEXT),
        2 => (PANEL, BORDER, BAD),
        _ => (PANEL, lerp(PANEL, BORDER, 0.6), lerp(DIM, PANEL, 0.35)),
    };
    let fill = if hot && kind != 3 { lerp(fill, TEXT, if kind == 0 { 0.14 } else { 0.07 }) } else { fill };
    let line = if hot && kind == 2 { BAD } else if kind == 0 { fill } else { line };
    ui.round(b, 8.0, fill, line);
    ui.text(label, b, 13.5, 600, fg, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
}

pub(crate) unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc: RECT = zeroed();
    GetClientRect(hwnd, &mut rc);
    let (cw, ch) = (rc.right, rc.bottom);
    let mem = CreateCompatibleDC(hdc);
    let bmp = CreateCompatibleBitmap(hdc, cw, ch);
    let ob = SelectObject(mem, bmp);
    let s = window_scale(hdc);
    let bgb = CreateSolidBrush(BG);
    FillRect(mem, &rc, bgb);
    DeleteObject(bgb);
    let mut g = null_mut::<GpGraphics>();
    GdipCreateFromHDC(mem, &mut g);
    GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
    let ui = Ui { dc: mem, g, s };
    let w = cw as f32 / s;
    let hover = HOVER.get();
    let dialog = DIALOG.with_borrow(|d| d.as_ref().map(|d| (d.title.clone(), d.text.clone(), d.ok.clone(), d.danger)));
    let hot = |id: usize| hover == Some(id);
    let busy = BUSY.load(Relaxed);
    let job = JOB.load(Relaxed);
    let n = PLUGINS.len();
    let n_inst = ready().filter(|&i| INSTALLED[i].load(Relaxed)).count();

    // header: logo + name
    ui.shape([0.0, 0.0, w, 3.0], 0.0, argb(ACCENT, 255));
    let logo = [M, 30.0, M + 60.0, 90.0];
    ui.round(logo, 16.0, ACCENT, ACCENT);
    let inner = lerp(ACCENT, BG, 0.12);
    ui.round([logo[0] + 4.0, logo[1] + 4.0, logo[2] - 4.0, logo[3] - 4.0], 13.0, inner, inner);
    ui.text("W", [logo[0], logo[1] - 2.0, logo[2], logo[3]], 32.0, 900, rgb(255, 255, 255), DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    let tx = logo[2] + 18.0;
    ui.text("WinRust", [tx, 28.0, w - M, 64.0], 30.0, 700, TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
    ui.text(&ptf!("Оболочка Windows на Rust  \u{00B7}  установлено {n_inst} из {N_READY}", "A Windows shell in Rust  \u{00B7}  {n_inst} of {N_READY} installed", "Uma interface do Windows em Rust  \u{00B7}  {n_inst} de {N_READY} instalados"), [tx, 64.0, w - M, 88.0], 13.0, 400, DIM, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
    ui.text(pt!("ВХОДИТ В СОСТАВ", "INCLUDED", "INCLUÍDO"), [M + 4.0, LIST_TOP - 26.0, w - M, LIST_TOP - 6.0], 11.0, 700, DIM, DT_LEFT | DT_SINGLELINE | DT_VCENTER);

    ui.text(if ready().all(|i| SELECTED[i].load(Relaxed)) { pt!("Снять выбор", "Select none", "Desmarcar todos") } else { pt!("Выбрать все", "Select all", "Selecionar todos") }, all_box(w), 11.5, 600, ACCENT, DT_RIGHT | DT_VCENTER | DT_SINGLELINE);
    ui.text(&format!("Fork {}", FORK_VERSION.trim()), [M, 92.0, w - M - 250.0, 107.0], 11.0, 400, DIM, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
    let available = crate::fork_update::AVAILABLE.lock().unwrap().clone();
    let link = available.map_or_else(|| pt!("Проверить обновления", "Check fork updates", "Verificar atualização do fork").to_string(), |v| format!("{} {v}", pt!("Загрузить", "Download", "Baixar")));
    ui.text(&link, update_box(w), 11.0, 600, ACCENT, DT_RIGHT | DT_SINGLELINE | DT_VCENTER);
    // component list and explicitly selected extras
    let main_bottom = LIST_TOP + N_READY as f32 * ROW_H + 12.0;
    ui.round([M, LIST_TOP, w - M, main_bottom], 12.0, PANEL, BORDER);
    ui.text(pt!("Дополнительно", "Extras · optional", "Extras · opcionais"), [M + 4.0, main_bottom + 2.0, w - M, main_bottom + EXTRA_GAP], 11.0, 700, DIM, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    ui.round([M, main_bottom + EXTRA_GAP, w - M, LIST_TOP + LIST_H], 12.0, PANEL, BORDER);
    let pos = rows().position(|i| i == job); // where install_all is now
    let n_sel = chosen().count();
    let _n_sel_inst = chosen().filter(|&i| INSTALLED[i].load(Relaxed)).count();
    let gear = gear_box(w);
    for (k, i) in rows().enumerate() {
        let p = &PLUGINS[i];
        let r = row_box(k, w);
        let installed = INSTALLED[i].load(Relaxed);
        let working = busy && (job == i || (job == n && installed));
        let queued = busy && !installed && SELECTED[i].load(Relaxed) && (job == STARTING || ready().position(|j| j == job).is_some_and(|q| k > q));
        if k > 0 && k != N_READY {
            ui.shape([r[0] + 12.0, r[1], r[2] - 12.0, r[1] + 1.0 / s], 0.0, argb(BORDER, 255));
        }
        if working {
            let t = lerp(PANEL, ACCENT, 0.10);
            ui.round([r[0], r[1] + 3.0, r[2], r[3] - 3.0], 9.0, t, t);
        }
        let (cb, _) = check_box(k, w);
        let sel = SELECTED[i].load(Relaxed);
        ui.round(cb, 5.0, if sel { ACCENT } else { PANEL }, if sel { ACCENT } else { lerp(BORDER, TEXT, if hot(ID_CHECK + i) { 0.4 } else { 0.0 }) });
        if sel {
            ui.text_in("\u{E73E}", cb, 11.0, 700, rgb(255, 255, 255), DT_CENTER | DT_VCENTER | DT_SINGLELINE, winrust_common::style::icon_face());
        }
        let tile = [r[0] + 42.0, r[1] + 13.0, r[0] + 78.0, r[1] + 49.0];
        let lit = installed || working;
        let tint = if lit { lerp(PANEL, ACCENT, 0.20) } else { lerp(PANEL, TEXT, 0.06) };
        ui.round(tile, 9.0, tint, tint);
        ui.text_in(p.glyph, tile, 16.0, 400, if lit { ACCENT } else { TEXT }, DT_CENTER | DT_VCENTER | DT_SINGLELINE, winrust_common::style::icon_face());

        let (pill, pc) = if working {
            (if job == n || REMOVING.load(Relaxed) { pt!("Удаление…", "Removing…", "Removendo…") } else { pt!("Установка…", "Installing…", "Instalando…") }, ACCENT)
        } else if installed {
            (pt!("Установлен", "Installed", "Instalado"), GOOD)
        } else if queued {
            (pt!("В очереди", "Queued", "Na fila"), DIM)
        } else {
            (pt!("Не установлен", "Not installed", "Não instalado"), DIM)
        };
        let pr = r[2] - 14.0 - 32.0 - 10.0;
        let pw = ui.width(pill, 11.5, 600) + 18.0;
        let pb = [pr - pw, r[1] + 21.0, pr, r[1] + 41.0];
        let ptint = lerp(PANEL, pc, 0.16);
        ui.round(pb, 10.0, ptint, ptint);
        ui.text(pill, pb, 11.5, 600, pc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

        let nx = tile[2] + 14.0;
        ui.text(p.name, [nx, r[1] + 11.0, pb[0] - 10.0, r[1] + 32.0], 14.5, 600, TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        let desc = p.desc.get();
        let short = if !p.bulk { if p.name == "purge" { pt!("НЕОБРАТИМО", "EXTRA · IRREVERSIBLE", "EXTRA · IRREVERSÍVEL") } else { pt!("Дополнительно", "EXTRA · changes system settings", "EXTRA · altera configurações do sistema") } } else { desc.split(". ").next().unwrap_or(desc).trim_end_matches('.') };
        ui.text(short, [nx, r[1] + 32.0, pb[0] - 10.0, r[3] - 10.0], 12.0, 400, DIM, DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS);

        if let Some((_, gb)) = gear.filter(|&(gi, _)| gi == i) {
            let h = hot(ID_GEAR) && !busy;
            ui.round(gb, 8.0, if h { lerp(PANEL, TEXT, 0.08) } else { PANEL }, BORDER);
            ui.text_in("\u{E713}", gb, 14.0, 400, if busy { DIM } else { TEXT }, DT_CENTER | DT_VCENTER | DT_SINGLELINE, winrust_common::style::icon_face());
        }
    }

    // the one button: installs everything that is missing (or updates all when nothing is)
    let mb = main_box(w);
    if busy {
        let base = lerp(PANEL, ACCENT, 0.22);
        ui.round(mb, 12.0, base, base);
        let done = if job == n { 0.0 } else { pos.map_or(0.0, |q| (q as f32 + 0.5) / rows().count().max(1) as f32) };
        if done > 0.0 {
            ui.shape([mb[0], mb[1], mb[0] + (mb[2] - mb[0]) * done, mb[3]], 12.0, argb(ACCENT, 255));
        }
        let label = if job == n {
            pt!("Удаление WinRust…", "Removing WinRust…", "Removendo WinRust…").to_string()
        } else {
            pos.map_or(pt!("Подготовка…", "Preparing…", "Preparando…").into(), |_| if REMOVING.load(Relaxed) { format!("{} {}", pt!("Удаление", "Removing", "Removendo"), PLUGINS[job].name) } else { format!("{} {}", pt!("Установка", "Installing", "Instalando"), PLUGINS[job].name) })
        };
        ui.text(&label, mb, 15.5, 700, TEXT, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    } else {
        let (remove, add) = planned();
        let all = remove.is_empty() && add.is_empty();
        let label = if all { pt!("Обновить выбранные", "Update installed", "Atualizar instalados").to_string() }
            else { pt!("Применить изменения", "Apply changes", "Aplicar alterações").to_string() };
        let (fill, fg) = if all { (PANEL, if n_sel == 0 { DIM } else { TEXT }) } else { (ACCENT, rgb(255, 255, 255)) };
        let fill = if hot(ID_MAIN) { lerp(fill, TEXT, if all { 0.07 } else { 0.14 }) } else { fill };
        ui.round(mb, 12.0, fill, if all { BORDER } else { fill });
        let tw = ui.width(&label, 16.0, 700);
        let cx = (mb[0] + mb[2]) / 2.0 + 13.0;
        ui.text_in(if all { "\u{E72C}" } else { "\u{E896}" }, [cx - tw / 2.0 - 30.0, mb[1], cx - tw / 2.0 - 6.0, mb[3]], 15.0, 400, fg, DT_CENTER | DT_VCENTER | DT_SINGLELINE, winrust_common::style::icon_face());
        ui.text(&label, [cx - tw / 2.0, mb[1], cx + tw / 2.0 + 4.0, mb[3]], 16.0, 700, fg, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    }

    // footer: status on the left, remove-all on the right
    let ft = WIN_H - FOOT_H;
    ui.shape([M, ft, w - M, ft + 1.0 / s], 0.0, argb(BORDER, 255));
    let (m, ok) = MSG.lock().unwrap().clone();
    let (m, dot, fg) = if busy {
        (pt!("Выполняется, не закрывайте окно…", "Working, do not close the window…", "Trabalhando, não feche a janela…").to_string(), ACCENT, DIM)
    } else if m.is_empty() {
        (pt!("Перед установкой создаётся точка восстановления.", "A restore point is created before installing.", "Um ponto de restauração é criado antes da instalação.").into(), DIM, DIM)
    } else if ok {
        (m, GOOD, TEXT)
    } else {
        (m, BAD, BAD)
    };
    let rb = rollback_box(w);
    ui.shape([M, ft + FOOT_H / 2.0 - 4.0, M + 8.0, ft + FOOT_H / 2.0 + 4.0], 4.0, argb(dot, 255));
    let one_line = ui.width(&m, 12.5, 400) < rb[0] - 16.0 - M - 18.0;
    let tb = [M + 18.0, ft + if one_line { 0.0 } else { 12.0 }, rb[0] - 16.0, WIN_H - if one_line { 0.0 } else { 8.0 }];
    ui.text(&m, tb, 12.5, 400, fg, DT_LEFT | DT_END_ELLIPSIS | if one_line { DT_SINGLELINE | DT_VCENTER } else { DT_WORDBREAK });
    button(&ui, rb, pt!("Удалить всё", "Remove all", "Remover tudo"), if n_inst > 0 && !busy { 2 } else { 3 }, hot(ID_ROLLBACK));
    // shows the language you would switch to; the other WinRust programs pick the choice up when they start
    button(&ui, lang_box(w), t!("Языки", "Languages"), 1, hot(ID_LANG));

    if LANGUAGE_MENU.get() {
        ui.shape([0.0, 0.0, w, WIN_H + 1.0], 0.0, argb(0, 160));
        let (d, cancel, apply) = dialog_boxes(w);
        ui.round(d, 14.0, lerp(PANEL, BG, 0.25), BORDER);
        ui.text(t!("Языки WinRust", "WinRust languages"), [d[0]+24.0,d[1]+18.0,d[2]-24.0,d[1]+48.0], 17.0, 600, TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        ui.text(t!("Язык интерфейса", "Display language"), [d[0]+24.0,d[1]+55.0,d[2]-24.0,d[1]+85.0], 12.0, 600, DIM, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        let names = ["English", "Русский", "Português (Brasil)", "日本語"];
        for (i, name) in names.iter().enumerate() {
            let b = locale_choice_box(w, i);
            let id = ID_LP_EN + i;
            let available = i < 1 || pack_available(i-1);
            button(&ui, b, name, if !available {3} else if LANGUAGE_SELECTED.get()==i {0} else {1}, hot(id));
        }
        ui.text(t!("Языковые пакеты (из архива с языками)", "Optional language packs (from the languages ZIP)"), [d[0]+24.0,d[1]+174.0,d[2]-24.0,d[1]+202.0], 12.0, 600, DIM, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        for (i, (_, name)) in crate::language_packs::PACKS.iter().enumerate() {
            let b = pack_choice_box(w, i);
            let checked = PACK_SELECTED.get()[i];
            let label = format!("{}  {}{}", if checked {"☑"} else {"☐"}, name, if pack_available(i) {""} else {t!(" (недоступен)", " (unavailable)")});
            ui.text(&label, b, 13.0, 400, if pack_available(i) {TEXT} else {DIM}, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        }
        let error = LANGUAGE_ERROR.with_borrow(|e| e.clone());
        if !error.is_empty() { ui.text(&error, [d[0]+24.0,d[1]+306.0,d[2]-24.0,d[1]+328.0], 11.5, 400, BAD, DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS); }
        button(&ui, cancel, t!("Отмена", "Cancel"), 1, hot(ID_PACK_CLOSE));
        button(&ui, apply, t!("Применить", "Apply"), 0, hot(ID_PACK_APPLY));
    }


    // confirmation dialog over the dimmed window
    if let Some((title, text, ok_label, danger)) = dialog {
        ui.shape([0.0, 0.0, w, WIN_H + 1.0], 0.0, argb(0, 160));
        let (d, cancel, okb) = dialog_boxes(w);
        ui.round(d, 14.0, lerp(PANEL, BG, 0.25), BORDER);
        let accent = if danger { BAD } else { ACCENT };
        let icon = [d[0] + 24.0, d[1] + 24.0, d[0] + 60.0, d[1] + 60.0];
        let tint = lerp(PANEL, accent, 0.18);
        ui.round(icon, 18.0, tint, tint);
        ui.text_in(if danger { "\u{E7BA}" } else { "\u{E946}" }, icon, 16.0, 400, accent, DT_CENTER | DT_VCENTER | DT_SINGLELINE, winrust_common::style::icon_face());
        ui.text(&title, [icon[2] + 14.0, d[1] + 24.0, d[2] - 24.0, d[1] + 60.0], 17.0, 600, TEXT, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        ui.text(&text, [d[0] + 24.0, d[1] + 76.0, d[2] - 24.0, cancel[1] - 12.0], 12.5, 400, DIM, DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS);
        button(&ui, cancel, pt!("Отмена", "Cancel", "Cancelar"), 1, hot(ID_CANCEL));
        let okfill = if hot(ID_OK) { lerp(accent, TEXT, 0.14) } else { accent };
        ui.round(okb, 8.0, okfill, okfill);
        ui.text(&ok_label, okb, 13.5, 600, rgb(255, 255, 255), DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    // settings panel of a plugin (topbar: OneDrive switch)
    if let Some(i) = SETTINGS.get() {
        ui.shape([0.0, 0.0, w, WIN_H + 1.0], 0.0, argb(0, 160));
        let (d, _, closeb) = dialog_boxes(w);
        ui.round(d, 14.0, lerp(PANEL, BG, 0.25), BORDER);
        let icon = [d[0] + 24.0, d[1] + 24.0, d[0] + 60.0, d[1] + 60.0];
        let tint = lerp(PANEL, ACCENT, 0.18);
        ui.round(icon, 18.0, tint, tint);
        ui.text_in("\u{E713}", icon, 16.0, 400, ACCENT, DT_CENTER | DT_VCENTER | DT_SINGLELINE, winrust_common::style::icon_face());
        ui.text(&ptf!("Настройки {}", "{} settings", "Configurações {}", PLUGINS[i].name), [icon[2] + 14.0, d[1] + 24.0, d[2] - 24.0, d[1] + 60.0], 17.0, 600, TEXT, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        let row = [d[0] + 24.0, d[1] + 78.0, d[2] - 24.0, d[1] + 148.0];
        let hid = od_hidden();
        ui.round(row, 10.0, PANEL, BORDER);
        ui.text(pt!("Отключение OneDrive", "Disable OneDrive", "Desabilitar OneDrive"), [row[0] + 16.0, row[1] + 10.0, row[2] - 80.0, row[1] + 32.0], 14.0, 600, TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        ui.text(pt!("Значок OneDrive не показывается в топбаре (по умолчанию). Синхронизация продолжает работать.", "The OneDrive icon is not shown in the bar (default). Sync keeps working.", "O ícone do OneDrive não é mostrado na barra (padrão). A sincronização continua funcionando."), [row[0] + 16.0, row[1] + 32.0, row[2] - 80.0, row[3] - 6.0], 12.0, 400, DIM, DT_LEFT | DT_WORDBREAK);
        let sw = switch_box(w);
        let off = lerp(PANEL, TEXT, 0.25);
        let track = if hid { ACCENT } else { off };
        ui.round(sw, 12.0, track, track);
        let kx = if hid { sw[2] - 12.0 } else { sw[0] + 12.0 };
        ui.round([kx - 8.0, sw[1] + 4.0, kx + 8.0, sw[3] - 4.0], 8.0, TEXT, TEXT);
        let row2 = [row[0], row[1] + 80.0, row[2], row[3] + 80.0];
        let on = explorer_idx().is_some_and(|e| SELECTED[e].load(Relaxed));
        ui.round(row2, 10.0, PANEL, BORDER);
        ui.text(pt!("Оболочка: rust-explorer", "Shell: rust-explorer", "Shell: rust-explorer"), [row2[0] + 16.0, row2[1] + 10.0, row2[2] - 80.0, row2[1] + 32.0], 14.0, 600, TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        ui.text(pt!("Включено: заменить Проводник. Выключено: остаётся стандартный Проводник Windows.", "On: replace Explorer. Off: the standard Windows Explorer stays.", "Ativado: substitua o Explorer. Desligado: o Windows Explorer padrão permanece."), [row2[0] + 16.0, row2[1] + 32.0, row2[2] - 80.0, row2[3] - 6.0], 12.0, 400, DIM, DT_LEFT | DT_WORDBREAK);
        let sw2 = shell_switch_box(w);
        let track2 = if on { ACCENT } else { off };
        ui.round(sw2, 12.0, track2, track2);
        let kx2 = if on { sw2[2] - 12.0 } else { sw2[0] + 12.0 };
        ui.round([kx2 - 8.0, sw2[1] + 4.0, kx2 + 8.0, sw2[3] - 4.0], 8.0, TEXT, TEXT);
        button(&ui, closeb, pt!("Закрыть", "Close", "Fechar"), 1, hot(ID_CANCEL));
    }

    GdipDeleteGraphics(g);
    BitBlt(hdc, 0, 0, cw, ch, mem, 0, 0, SRCCOPY);
    SelectObject(mem, ob);
    DeleteObject(bmp);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}

// ---------- window ----------

/// What a confirmed dialog runs.
#[derive(Clone, Copy)]
pub(crate) enum Act {
    InstallAll,
    Rollback,
}
pub(crate) struct Dialog {
    title: String,
    text: String,
    ok: String,
    danger: bool,
    act: Act,
}
thread_local! { pub(crate) static DIALOG: std::cell::RefCell<Option<Dialog>> = const { std::cell::RefCell::new(None) }; }
thread_local! {
    static LANGUAGE_MENU: Cell<bool> = const { Cell::new(false) };
    static LANGUAGE_SELECTED: Cell<usize> = const { Cell::new(0) };
    static PACK_SELECTED: Cell<[bool; 3]> = const { Cell::new([false; 3]) };
    static PACK_AVAILABLE: Cell<[bool; 3]> = const { Cell::new([false; 3]) };
    static LANGUAGE_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}
const LANGUAGES: [&str; 4] = ["en", "ru", "pt-BR", "ja"];
fn probe_pack(i: usize) -> bool {
    let Some(&(code, _)) = crate::language_packs::PACKS.get(i) else { return false };
    crate::language_packs::source_dir().is_some_and(|d| crate::language_packs::valid_in(&d, code)) ||
        crate::language_packs::installed_dir().is_some_and(|d| crate::language_packs::valid_in(&d, code))
}
fn pack_available(i: usize) -> bool { PACK_AVAILABLE.get().get(i).copied().unwrap_or(false) }

/// Clickable thing under the cursor (ID_*). While a panel is up only its buttons count; while a job runs nothing does.
pub(crate) unsafe fn hit(hwnd: HWND, lp: isize) -> Option<usize> {
    let (x, y) = ((lp & 0xFFFF) as i16 as f32, ((lp >> 16) & 0xFFFF) as i16 as f32);
    let (mut rc, dc): (RECT, _) = (zeroed(), GetDC(hwnd));
    let s = window_scale(dc);
    ReleaseDC(hwnd, dc);
    GetClientRect(hwnd, &mut rc);
    let w = rc.right as f32 / s;
    let inside = |b: Box4| x >= b[0] * s && x <= b[2] * s && y >= b[1] * s && y <= b[3] * s;
    if LANGUAGE_MENU.get() {
        let (_, cancel, apply) = dialog_boxes(w);
        for i in 0..4 { if inside(locale_choice_box(w, i)) { return Some(ID_LP_EN + i); } }
        for i in 0..3 { if inside(pack_choice_box(w, i)) { return Some(ID_PACK_PT + i); } }
        return if inside(apply) { Some(ID_PACK_APPLY) } else if inside(cancel) { Some(ID_PACK_CLOSE) } else { None };
    }
    if SETTINGS.get().is_some() {
        let (_, _, closeb) = dialog_boxes(w);
        return if inside(switch_box(w)) { Some(ID_SWITCH) } else if inside(shell_switch_box(w)) { Some(ID_SHELL) } else if inside(closeb) { Some(ID_CANCEL) } else { None };
    }
    if DIALOG.with_borrow(|d| d.is_some()) {
        let (_, cancel, ok) = dialog_boxes(w);
        return if inside(ok) { Some(ID_OK) } else if inside(cancel) { Some(ID_CANCEL) } else { None };
    }
    if BUSY.load(Relaxed) {
        return None;
    }
    if gear_box(w).is_some_and(|(_, b)| inside(b)) {
        return Some(ID_GEAR);
    }
    if inside(update_box(w)) { return Some(ID_UPDATE); }
    if inside(all_box(w)) { return Some(ID_ALL); }
    if let Some(i) = rows().enumerate().find_map(|(k, i)| inside(check_box(k, w).1).then_some(i)) {
        return Some(ID_CHECK + i);
    }
    if inside(lang_box(w)) {
        return Some(ID_LANG);
    }
    if inside(main_box(w)) {
        return Some(ID_MAIN);
    }
    (INSTALLED.iter().any(|f| f.load(Relaxed)) && inside(rollback_box(w))).then_some(ID_ROLLBACK)
}

/// Runs `job` off the UI thread; its Ok/Err text becomes the status line. `which` = plugin index, n = rollback.
pub(crate) fn start(hwnd: HWND, which: usize, job: impl FnOnce() -> Result<String, String> + Send + 'static) {
    JOB.store(which, Relaxed);
    REMOVING.store(false, Relaxed);
    BUSY.store(true, Relaxed);
    HOVER.set(None);
    unsafe { InvalidateRect(hwnd, null(), 0) };
    let h = hwnd as isize;
    thread::spawn(move || {
        *MSG.lock().unwrap() = match job() {
            Ok(m) => (m, true),
            Err(e) => (ptf!("Ошибка: {e}", "Error: {e}", "Erro: {e}"), false),
        };
        refresh();
        BUSY.store(false, Relaxed);
        unsafe { PostMessageW(h as HWND, WM_DONE, 0, 0) };
    });
}

/// One click: every component in order; missing ones are installed, installed ones updated from this installer.
/// Only the ticked ones. A failed or skipped one does not stop the others (rust-explorer refuses by itself while topbar is
/// missing); the failures are listed at the end. `h` = window to repaint between steps.
pub(crate) fn install_all(h: isize) -> Result<String, String> {
    let (remove, add) = planned();
    crate::fork_update::reject_older_installer()?;
    let mut done = Vec::new(); let mut failed = Vec::new(); let mut skipped = Vec::new(); let mut warned = false;
    for i in remove {
        REMOVING.store(true, Relaxed);
        JOB.store(i, Relaxed);
        unsafe { InvalidateRect(h as HWND, null(), 0) };
        if PLUGINS[i].name == "topbar" && PLUGINS.iter().find(|p| p.name == RUST_EXPLORER).is_some_and(|p| (p.installed)()) {
            failed.push("topbar: rust-explorer is still installed".to_string()); continue;
        }
        match uninstall_plugin(i) { Ok(_) => done.push(PLUGINS[i].name), Err(e) => failed.push(format!("{}: {e}", PLUGINS[i].name)) }
        refresh();
    }
    let install: Vec<_> = rows().filter(|&i| add.contains(&i) || (PLUGINS[i].bulk && INSTALLED[i].load(Relaxed) && SELECTED[i].load(Relaxed))).collect();
    for i in install {
        REMOVING.store(false, Relaxed);
        JOB.store(i, Relaxed);
        unsafe { InvalidateRect(h as HWND, null(), 0) };
        if INSTALLED[i].load(Relaxed) && component_current(PLUGINS[i].name) { skipped.push(PLUGINS[i].name); continue; }
        match install_plugin(i) {
            Ok(_) => { warned |= !WARN.lock().unwrap().is_empty(); done.push(PLUGINS[i].name); }
            Err(e) => failed.push(format!("{}: {e}", PLUGINS[i].name)),
        }
        refresh();
    }
    if !failed.is_empty() { return Err(format!("{}; completed: {}", failed.join("; "), done.join(", "))); }
    if ready().any(|i| INSTALLED[i].load(Relaxed)) { crate::fork_update::mark_installed().map_err(|e| format!("Components processed, but version registration failed: {e}"))?; }
    let mut result = if done.is_empty() { pt!("Изменений нет", "No changes needed", "Nenhuma alteração necessária").to_string() } else { format!("{}: {}", pt!("Готово", "Completed", "Concluído"), done.join(", ")) };
    if !skipped.is_empty() { result.push_str(&format!("; {}: {}", pt!("Актуально", "Already up to date", "Já atualizados"), skipped.join(", "))); }
    if warned { result.push_str(pt!(" Проверьте журнал.", " Check the install log for warnings.", " Confira os avisos no registro de instalação.")); }
    Ok(result)
}

pub(crate) unsafe fn ask(hwnd: HWND, d: Dialog) {
    DIALOG.set(Some(d));
    HOVER.set(None);
    InvalidateRect(hwnd, null(), 0);
}

pub(crate) unsafe fn close_dialog(hwnd: HWND, run: bool) {
    let Some(d) = DIALOG.take() else { return };
    HOVER.set(None);
    InvalidateRect(hwnd, null(), 0);
    if !run {
        return;
    }
    match d.act {
        Act::Rollback => start(hwnd, PLUGINS.len(), rollback),
        Act::InstallAll => {
            let h = hwnd as isize;
            start(hwnd, STARTING, move || install_all(h))
        }
    }
}

pub(crate) unsafe fn click(hwnd: HWND, id: usize) {
    if LANGUAGE_MENU.get() {
        match id {
            ID_PACK_CLOSE => LANGUAGE_MENU.set(false),
            ID_LP_EN..=ID_LP_JA => {
                let i = id - ID_LP_EN;
                if i < 1 || pack_available(i - 1) {
                    LANGUAGE_SELECTED.set(i);
                    if i >= 1 { let mut selected = PACK_SELECTED.get(); selected[i-1] = true; PACK_SELECTED.set(selected); }
                }
            }
            ID_PACK_PT..=ID_PACK_JA => {
                let i = id - ID_PACK_PT;
                if pack_available(i) { let mut selected = PACK_SELECTED.get(); selected[i] = !selected[i]; PACK_SELECTED.set(selected); }
            }
            ID_PACK_APPLY => {
                let selected = PACK_SELECTED.get();
                let language = LANGUAGES[LANGUAGE_SELECTED.get()];
                LANGUAGE_MENU.set(false);
                start(hwnd, STARTING, move || {
                    let session = init()?;
                    if session.other { return Err(t!("Используйте учётную запись пользователя сеанса.", "Use the signed-in user's administrator account for language settings.").into()); }
                    let source = crate::language_packs::source_dir().ok_or(t!("Путь установщика недоступен", "Installer location unavailable"))?;
                    let target = crate::language_packs::installed_dir().ok_or(t!("ProgramData недоступна", "ProgramData unavailable"))?;
                    let codes: Vec<_> = crate::language_packs::PACKS.iter().enumerate().filter_map(|(i, (id, _))| selected[i].then_some(*id)).collect();
                    crate::language_packs::install(&source, &target, &codes)?;
                    if matches!(language, "ru" | "pt-BR" | "ja") && !crate::language_packs::valid_in(&target, language) {
                        return Err(t!("Языковой пакет не установлен", "The selected language pack is not installed").into());
                    }
                    i18n::locales::retry_missing();
                    i18n::set_language(language).map_err(|e| e.to_string())?;
                    Ok(t!("Языки сохранены. Перезапустите приложения, чтобы обновить переводы.", "Languages saved. Restart running applications to refresh translations.").into())
                });
                return;
            }
            _ => {}
        }
        HOVER.set(None); InvalidateRect(hwnd, null(), 0); return;
    }
    if SETTINGS.get().is_some() {
        if id == ID_SWITCH {
            if let Err(e) = set_od_hidden(!od_hidden()) {
                *MSG.lock().unwrap() = (ptf!("Ошибка: {e}", "Error: {e}", "Erro: {e}"), false);
            }
        } else if id == ID_SHELL {
            if let Some(e) = explorer_idx() {
                toggle_component(e);
            }
        } else if id == ID_CANCEL {
            SETTINGS.set(None);
        }
        HOVER.set(None);
        InvalidateRect(hwnd, null(), 0);
        return;
    }
    if id == ID_OK || id == ID_CANCEL {
        return close_dialog(hwnd, id == ID_OK);
    }
    if BUSY.load(Relaxed) {
        return;
    }
    match id {
        i if i >= ID_CHECK => {
            toggle_component(i - ID_CHECK);
            InvalidateRect(hwnd, null(), 0);
        }
        ID_UPDATE => {
            let available = crate::fork_update::AVAILABLE.lock().unwrap().is_some();
            start(hwnd, STARTING, move || {
                if available { crate::fork_update::download_and_open() }
                else { match crate::fork_update::check() {
                    Ok(Some(v)) => Ok(format!("{} {v}", pt!("Доступно", "Update available", "Atualização disponível"))),
                    Ok(None) => Ok(pt!("Обновлений нет", "No newer signed fork release", "Nenhuma release assinada mais recente do fork").into()),
                    Err(e) => Err(format!("{}: {e}", pt!("Проверка обновлений не удалась", "Fork update check failed", "Falha ao verificar atualização do fork"))),
                }}
            });
        }
        ID_ALL => {
            let next = !ready().all(|i| SELECTED[i].load(Relaxed));
            for i in ready() { SELECTED[i].store(next, Relaxed); }
            if let (Some(e), Some(b)) = (explorer_idx(), PLUGINS.iter().position(|p| p.name == "topbar")) {
                if SELECTED[e].load(Relaxed) { SELECTED[b].store(true, Relaxed); }
            }
            InvalidateRect(hwnd, null(), 0);
        }
        ID_MAIN => {
            let (remove, add) = planned();
            let mut text = String::new();
            if add.iter().any(|&i| PLUGINS[i].name == "purge") {
                text.push_str(pt!("НЕОБРАТИМО: приложения удаляются для всех пользователей. Отката нет.", "IRREVERSIBLE: apps will be removed for all users. There is no undo.", "IRREVERSÍVEL: aplicativos serão removidos para todos os usuários. Não há como desfazer."));
                text.push_str("\n\n");
            }
            if remove.iter().any(|&i| PLUGINS[i].name == "purge") {
                text.push_str(pt!("Удаление не восстанавливает приложения.", "Removing purge does not restore applications it deleted.", "Remover o purge não recupera os aplicativos que ele apagou."));
                text.push_str("\n\n");
            }
            text.push_str(&format!("{}: {}\n{}: {}", pt!("Удалить", "Remove", "Remover"), remove.iter().map(|&i| PLUGINS[i].name).collect::<Vec<_>>().join(", "), pt!("Установить", "Install", "Instalar"), add.iter().map(|&i| PLUGINS[i].name).collect::<Vec<_>>().join(", ")));
            for &i in &add {
                if PLUGINS[i].name == "debloat" { text.push_str("\n\n"); text.push_str(pt!("Debloat меняет службы и политики.", "Debloat changes services and policies.", "Debloat altera serviços e políticas.")); }
                if PLUGINS[i].name == RUST_EXPLORER { text.push_str("\n\n"); text.push_str(pt!("Explorer может заменить оболочку и изменить InventorySvc, lfsvc, DoSvc и svchost.", "Explorer may replace the shell and change InventorySvc, lfsvc, DoSvc and svchost settings.", "Explorer pode substituir a interface e alterar InventorySvc, lfsvc, DoSvc e svchost.")); }
            }
            if remove.is_empty() && add.is_empty() { text = pt!("Выбранные файлы будут сравнены с установщиком.", "Installed components will be compared with this installer and only changed files updated.", "Os componentes instalados serão comparados com este instalador e apenas os arquivos diferentes serão atualizados.").into(); }
            let danger = !remove.is_empty() || add.iter().any(|&i| PLUGINS[i].name == "purge");
            ask(hwnd, Dialog { title: pt!("Подтвердите изменения", "Confirm changes", "Confirmar alterações").into(), text, ok: pt!("Применить", "Apply", "Aplicar").into(), danger, act: Act::InstallAll });
        }
        ID_LANG => {
            let current = i18n::language();
            LANGUAGE_SELECTED.set(LANGUAGES.iter().position(|&x| x == current).unwrap_or(0));
            let installed = crate::language_packs::installed_dir();
            PACK_AVAILABLE.set(std::array::from_fn(probe_pack));
            PACK_SELECTED.set(std::array::from_fn(|i| {
                let id = crate::language_packs::PACKS[i].0;
                (current == id || installed.as_ref().is_some_and(|d| crate::language_packs::valid_in(d, id))) && pack_available(i)
            }));
            LANGUAGE_ERROR.set(String::new());
            LANGUAGE_MENU.set(true);

            HOVER.set(None);
            InvalidateRect(hwnd, null(), 0);
        }
        ID_GEAR => {
            SETTINGS.set(PLUGINS.iter().position(|p| p.settings));
            HOVER.set(None);
            InvalidateRect(hwnd, null(), 0);
        }
        ID_ROLLBACK => ask(hwnd, Dialog {
            title: pt!("Удалить WinRust?", "Remove WinRust?", "Remover WinRust?").into(),
            text: pt!("Все компоненты будут удалены. Вернутся поиск Windows, стандартный диспетчер задач, панель задач и обычный Проводник.", "All components will be removed. Windows Search, the stock Task Manager, the taskbar and regular Explorer come back.", "Todos os componentes serão removidos. O Windows Search, o Gerenciador de Tarefas padrão, a barra de tarefas e o Explorer normal voltam.").into(),
            ok: pt!("Удалить всё", "Remove all", "Remover tudo").into(),
            danger: true,
            act: Act::Rollback,
        }),
        _ => {}
    }
}

pub(crate) unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => paint(hwnd),
        WM_ERASEBKGND => return 1,
        WM_MOUSEMOVE => {
            let h = hit(hwnd, lp);
            if h != HOVER.get() {
                HOVER.set(h);
                InvalidateRect(hwnd, null(), 0);
            }
            let mut t = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
            TrackMouseEvent(&mut t);
        }
        WM_SETCURSOR if (lp & 0xFFFF) as u32 == HTCLIENT => {
            SetCursor(LoadCursorW(null_mut(), if HOVER.get().is_some() { IDC_HAND } else { IDC_ARROW }));
            return 1;
        }
        WM_MOUSELEAVE => {
            HOVER.set(None);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_LBUTTONUP => {
            if let Some(i) = hit(hwnd, lp) {
                click(hwnd, i);
            }
        }
        WM_KEYDOWN if LANGUAGE_MENU.get() && wp as u16 == VK_ESCAPE => {
            LANGUAGE_MENU.set(false);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_KEYDOWN if SETTINGS.get().is_some() && wp as u16 == VK_ESCAPE => {
            SETTINGS.set(None);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_KEYDOWN if DIALOG.with_borrow(|d| d.is_some()) => match wp as u16 {
            VK_ESCAPE => close_dialog(hwnd, false),
            VK_RETURN if DIALOG.with_borrow(|d| d.as_ref().is_some_and(|d| !d.danger)) => close_dialog(hwnd, true), // a dangerous one needs the click

            _ => {}
        },
        WM_DONE => {
            InvalidateRect(hwnd, null(), 0);
        }
        WM_CLOSE if BUSY.load(Relaxed) => {} // closing mid-install would leave the system half-changed
        WM_DESTROY => PostQuitMessage(0),
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    }
    0
}

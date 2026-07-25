//! Wizard theme: colors, fonts, and the raw painting that gives every page a
//! white content canvas with a light-gray sidebar and accent-colored text.
//!
//! DPI coordinate rule (this is the bug the redesign fixes): with nwg's
//! `high-dpi` feature, control positions/sizes/font sizes go through nwg's
//! builders and are auto-scaled logical -> physical, so callers pass LOGICAL
//! values everywhere and never multiply by `scale_factor()`. The exception is
//! the `FillRect` calls in the WM_ERASEBKGND handler below: those run against
//! the raw device context in PHYSICAL pixels (nwg is not in that path), so the
//! sidebar width / divider Y constants ARE scaled here. Rule of thumb:
//! nwg-places-it = logical; I-draw-it-raw = physical.

use native_windows_gui as nwg;
use nwg::ControlHandle;
use std::cell::RefCell;
use std::sync::OnceLock;

use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, FillRect, SetBkColor, SetBkMode, SetTextColor, HBRUSH, HDC, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetParent, WM_CTLCOLORSTATIC, WM_ERASEBKGND,
};

pub const ACCENT: [u8; 3] = [15, 108, 189]; // #0F6CBD
pub const MUTED: [u8; 3] = [96, 94, 92]; // #605E5C
pub const WHITE: [u8; 3] = [255, 255, 255];
pub const SIDEBAR_BG: [u8; 3] = [243, 243, 247]; // #F3F3F7
pub const DIVIDER: [u8; 3] = [225, 223, 221]; // #E1DFDD

/// Near-black used for page titles / body labels (plan calls it "#1B1B1B via
/// default"); applied to any static that isn't registered muted or accent.
const NEAR_BLACK: [u8; 3] = [27, 27, 27];

// Logical layout constants shared with the shell (mod.rs) — scaled to physical
// inside the erase handler only.
const SIDEBAR_W: i32 = 240;
const DIVIDER_Y: i32 = 560;

// One arbitrary-but-fixed subclass id for our handler; we bind exactly one theme
// handler per parent, so a constant is fine.
const HANDLER_ID: usize = 0x0F6C_BD00;

fn colorref(c: [u8; 3]) -> COLORREF {
    COLORREF(c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16)
}

#[cfg(test)]
mod tests {
    #[test]
    fn colorref_packs_bgr() {
        // COLORREF is 0x00BBGGRR: red lands in the low byte.
        assert_eq!(super::colorref([255, 0, 0]).0, 0x0000FF);
        assert_eq!(super::colorref(super::ACCENT).0, 0x00BD6C0F);
    }
}

fn hwnd_key(h: &ControlHandle) -> isize {
    h.hwnd().map(|p| p as isize).unwrap_or(0)
}

// ---- Fonts -----------------------------------------------------------------

fn build_font(size: u32, weight: u32) -> nwg::Font {
    let mut f = nwg::Font::default();
    nwg::Font::builder()
        .family("Segoe UI")
        .size(size)
        .weight(weight)
        .build(&mut f)
        .expect("theme font");
    f
}

/// Segoe UI 15px, the wizard body font. Cached for the process lifetime.
pub fn body_font() -> &'static nwg::Font {
    static F: OnceLock<nwg::Font> = OnceLock::new();
    F.get_or_init(|| build_font(15, 400))
}

/// Segoe UI Semibold (weight 600) at the requested px size. Not cached — the
/// shell uses a couple of one-off sizes (sidebar title, active step rows).
pub fn semibold(size: u32) -> nwg::Font {
    build_font(size, 600)
}

/// Segoe UI Semibold 26px — the content-header page title.
pub fn title_font() -> &'static nwg::Font {
    static F: OnceLock<nwg::Font> = OnceLock::new();
    F.get_or_init(|| build_font(26, 600))
}

/// Segoe UI 14px — content-header subtitle / small muted text.
pub fn subtitle_font() -> &'static nwg::Font {
    static F: OnceLock<nwg::Font> = OnceLock::new();
    F.get_or_init(|| build_font(14, 400))
}

// ---- Text/background registries --------------------------------------------

#[derive(Default)]
struct Reg {
    accent_text: Vec<isize>,
    muted_text: Vec<isize>,
    gray_bg: Vec<isize>,   // sidebar labels: painted on SIDEBAR_BG, not white
    accent_bg: Vec<isize>, // the moving 3x24 active-step bar
    active_step: isize,    // step label that is currently the active page
}

thread_local! {
    static REG: RefCell<Reg> = RefCell::new(Reg::default());
}

struct Brushes {
    white: HBRUSH,
    gray: HBRUSH,
    accent: HBRUSH,
    divider: HBRUSH,
}

thread_local! {
    // Created once on first paint and never freed (GUI lives for the whole
    // process; leaking these is cheaper and simpler than tracking ownership).
    static BRUSHES: Brushes = unsafe {
        Brushes {
            white: CreateSolidBrush(colorref(WHITE)),
            gray: CreateSolidBrush(colorref(SIDEBAR_BG)),
            accent: CreateSolidBrush(colorref(ACCENT)),
            divider: CreateSolidBrush(colorref(DIVIDER)),
        }
    };
}

/// Gray muted text for this static (on white or on the sidebar).
pub fn register_muted(h: &ControlHandle) {
    let k = hwnd_key(h);
    REG.with(|r| r.borrow_mut().muted_text.push(k));
}

/// Accent-colored text for this static.
// ponytail: part of the theme API the page layout tasks (2-7) consume; no
// caller in the shell yet.
#[allow(dead_code)]
pub fn register_accent(h: &ControlHandle) {
    let k = hwnd_key(h);
    REG.with(|r| r.borrow_mut().accent_text.push(k));
}

/// Shell-only: this static sits in the sidebar, so paint its background with
/// SIDEBAR_BG instead of white.
pub fn register_sidebar_bg(h: &ControlHandle) {
    let k = hwnd_key(h);
    REG.with(|r| r.borrow_mut().gray_bg.push(k));
}

/// Shell-only: this (empty) static is the active-step accent bar; fill it solid
/// accent.
pub fn register_accent_bar(h: &ControlHandle) {
    let k = hwnd_key(h);
    REG.with(|r| r.borrow_mut().accent_bg.push(k));
}

/// Shell-only: mark which step label is the active page. Its text is drawn in
/// accent regardless of the muted registration all step rows carry.
pub fn set_active_step(h: &ControlHandle) {
    let k = hwnd_key(h);
    REG.with(|r| r.borrow_mut().active_step = k);
}

// ---- Painting --------------------------------------------------------------

/// Attach white-canvas painting + text coloring to a parent (the main window or
/// a page frame). Handles WM_CTLCOLORSTATIC (text/background per registry) and
/// WM_ERASEBKGND (fill white; on the main window also the gray sidebar strip
/// and the footer divider). Idempotent enough for our use: call once per parent.
pub fn attach(parent: &ControlHandle) {
    let raw = match parent.hwnd() {
        Some(h) => h,
        None => return,
    };
    // Top-level window (no parent) owns the sidebar + divider; frames don't.
    let is_window = unsafe { GetParent(HWND(raw as *mut core::ffi::c_void)) }
        .map(|p| p.0.is_null())
        .unwrap_or(true);

    // RawEventHandler has no Drop, so the subclass stays installed after this
    // returns — exactly what we want (process-lifetime painting). A failed
    // bind only costs theming (cosmetic), but leave a debug breadcrumb.
    let bound = nwg::bind_raw_event_handler(parent, HANDLER_ID, move |hwnd, msg, w, l| {
        match msg {
            WM_CTLCOLORSTATIC => {
                let child = l as isize;
                let hdc = HDC(w as *mut core::ffi::c_void);
                let (text, brush, bg) = REG.with(|r| {
                    let r = r.borrow();
                    let text = if child == r.active_step || r.accent_text.contains(&child) {
                        ACCENT
                    } else if r.muted_text.contains(&child) {
                        MUTED
                    } else {
                        NEAR_BLACK
                    };
                    let (brush, bg) = BRUSHES.with(|b| {
                        if r.accent_bg.contains(&child) {
                            (b.accent, ACCENT)
                        } else if r.gray_bg.contains(&child) {
                            (b.gray, SIDEBAR_BG)
                        } else {
                            (b.white, WHITE)
                        }
                    });
                    (text, brush, bg)
                });
                unsafe {
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, colorref(text));
                    SetBkColor(hdc, colorref(bg));
                }
                Some(brush.0 as isize)
            }
            WM_ERASEBKGND => {
                let hdc = HDC(w as *mut core::ffi::c_void);
                let wnd = HWND(hwnd as *mut core::ffi::c_void);
                let mut rc = RECT::default();
                if unsafe { GetClientRect(wnd, &mut rc) }.is_err() {
                    return None;
                }
                BRUSHES.with(|b| unsafe {
                    // Whole client white first (content canvas / frame body).
                    FillRect(hdc, &rc, b.white);
                    if is_window {
                        // Raw DC is in physical px: scale the logical constants.
                        let sf = nwg::scale_factor();
                        let side_w = (SIDEBAR_W as f64 * sf) as i32;
                        let div_y = (DIVIDER_Y as f64 * sf) as i32;
                        let side = RECT { left: 0, top: 0, right: side_w, bottom: rc.bottom };
                        FillRect(hdc, &side, b.gray);
                        let div = RECT { left: side_w, top: div_y, right: rc.right, bottom: div_y + 1 };
                        FillRect(hdc, &div, b.divider);
                    }
                });
                Some(1)
            }
            _ => None,
        }
    });
    debug_assert!(bound.is_ok(), "theme::attach: raw handler bind failed");
}

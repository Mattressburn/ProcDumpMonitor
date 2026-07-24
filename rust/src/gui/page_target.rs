use crate::config::{Config, TargetType};
use crate::services;
use native_windows_gui as nwg;
use std::cell::RefCell;
use std::rc::Rc;

use super::theme;

pub struct TargetPage {
    pub txt_process: nwg::TextInput,
    pub cmb_service: nwg::ComboBox<String>,
    pub chk_show_all: nwg::CheckBox,
    pub btn_refresh: nwg::Button,
    /// Parallel to `cmb_service`'s items.
    pub services: RefCell<Vec<services::ServiceInfo>>,
    pub picked_service: RefCell<Option<String>>,

    /// Static labels/hint text for this page. nwg destroys a control's HWND
    /// when its Rust value drops, so anything not stored somewhere on the
    /// page (here, or in a public field above) vanishes right after
    /// `build()` returns -- this is the bug that shipped originally (locals
    /// `lbl`, `lbl2`, `hint` dropped at the end of the old `build()`).
    #[allow(dead_code)]
    _labels: Vec<nwg::Label>,
}

// Design-system grid (see .superpowers/sdd/design-system.md), relative to
// the 680x456 page frame.
const LABEL_X: i32 = 32;
const LABEL_W: i32 = 190;
const FIELD_X: i32 = 232;
const FIELD_W: i32 = 408;
const ROW_PITCH: i32 = 40;
const ROW0_Y: i32 = 32; // PAD

fn mk_label(parent: &nwg::Frame, text: &str, pos: (i32, i32), size: (i32, i32)) -> nwg::Label {
    let mut l = nwg::Label::default();
    nwg::Label::builder().text(text).position(pos).size(size).parent(parent).build(&mut l).unwrap();
    l
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> TargetPage {
    let mut labels: Vec<nwg::Label> = Vec::new();

    // Row 1: "Process name" label + input.
    let row1_y = ROW0_Y;
    labels.push(mk_label(parent, "Process name", (LABEL_X, row1_y - 2), (LABEL_W, 20)));
    let mut txt_process = nwg::TextInput::default();
    nwg::TextInput::builder()
        .position((FIELD_X, row1_y))
        .size((FIELD_W, 26))
        .parent(parent)
        .build(&mut txt_process)
        .unwrap();

    // Row 2: "Or pick a service" label + combo.
    let row2_y = ROW0_Y + ROW_PITCH;
    labels.push(mk_label(parent, "Or pick a service", (LABEL_X, row2_y - 2), (LABEL_W, 20)));
    let mut cmb_service = nwg::ComboBox::default();
    nwg::ComboBox::builder()
        .position((FIELD_X, row2_y))
        .size((FIELD_W, 26))
        .parent(parent)
        .build(&mut cmb_service)
        .unwrap();

    // Row 3: checkbox + refresh button side by side in the field column.
    // Widths are sized off measured native control preferred-sizes (Segoe UI
    // 15px: checkbox text+glyph ~275px, button text+chrome ~124px) plus a
    // small buffer, so both sit within the field column's ~416px without
    // clipping while keeping the design system's >=32px right margin.
    let row3_y = ROW0_Y + ROW_PITCH * 2;
    const CHK_W: i32 = 280;
    const CHK_BTN_GAP: i32 = 8;
    const BTN_W: i32 = 128;
    const BTN_H: i32 = 32;
    const CHK_H: i32 = 26;
    let mut chk_show_all = nwg::CheckBox::default();
    nwg::CheckBox::builder()
        .text("Show all services (including stopped)")
        .position((FIELD_X, row3_y + (BTN_H - CHK_H) / 2))
        .size((CHK_W, CHK_H))
        .parent(parent)
        .build(&mut chk_show_all)
        .unwrap();
    let mut btn_refresh = nwg::Button::default();
    nwg::Button::builder()
        .text("Refresh services")
        .position((FIELD_X + CHK_W + CHK_BTN_GAP, row3_y))
        .size((BTN_W, BTN_H))
        .parent(parent)
        .build(&mut btn_refresh)
        .unwrap();

    // Former hint sentence, now muted hint text 8px under row 3.
    let hint_y = row3_y + BTN_H + 8;
    let hint = mk_label(
        parent,
        "Picking a service fills the name and targets it as a service; typing targets a process.",
        (LABEL_X, hint_y),
        (616, 24),
    );
    theme::register_muted(&hint.handle);
    labels.push(hint);

    let page = TargetPage {
        txt_process,
        cmb_service,
        chk_show_all,
        btn_refresh,
        services: RefCell::new(Vec::new()),
        picked_service: RefCell::new(None),
        _labels: labels,
    };
    page.refresh_services();
    page
}

impl TargetPage {
    pub fn refresh_services(&self) {
        let all = services::list();
        let show_all = self.chk_show_all.check_state() == nwg::CheckBoxState::Checked;
        let filtered: Vec<services::ServiceInfo> =
            all.into_iter().filter(|s| show_all || s.running).collect();
        let labels = filtered.iter().map(|s| format!("{} ({})", s.display, s.name)).collect();
        self.cmb_service.set_collection(labels);
        *self.services.borrow_mut() = filtered;
    }

    /// Wired into gui::run's event handler:
    /// - OnComboxBoxSelection on cmb_service -> on_service_picked()
    /// - OnButtonClick on btn_refresh and on chk_show_all -> refresh_services()
    pub fn on_service_picked(&self) {
        if let Some(i) = self.cmb_service.selection() {
            if let Some(svc) = self.services.borrow().get(i) {
                self.txt_process.set_text(&svc.name);
                *self.picked_service.borrow_mut() = Some(svc.name.clone());
            }
        }
    }

    pub fn load(&self, cfg: &Config) {
        self.txt_process.set_text(&cfg.target_name);
        *self.picked_service.borrow_mut() = match cfg.target_type {
            TargetType::Service => Some(cfg.target_name.clone()),
            TargetType::Process => None,
        };
    }

    /// Always succeeds -- Target never blocks navigation. Returns `bool` to
    /// match the page contract every page shares (only Notify's `save` ever
    /// returns `false`, on invalid email settings).
    pub fn save(&self, cfg: &mut Config) -> bool {
        let typed = self.txt_process.text().trim().to_string();
        // If the text still equals the last-picked service name, the user
        // hasn't retyped over it -> still targeting that service. Decision
        // lives in config::infer_target_type so it's unit-tested on Linux
        // (this file only compiles on Windows).
        let picked = self.picked_service.borrow();
        cfg.target_type = crate::config::infer_target_type(&typed, picked.as_deref());
        // Auto task name follows target when the user hasn't customized it
        // (Task 10's Task page re-checks this same condition).
        if cfg.task_name == crate::task::auto_task_name(&cfg.target_name)
            || cfg.task_name == "ProcDump Monitor"
        {
            cfg.task_name = crate::task::auto_task_name(&typed);
        }
        cfg.target_name = typed;
        true
    }
}

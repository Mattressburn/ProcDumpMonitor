use crate::config::Config;
use native_windows_gui as nwg;
use std::rc::Rc;

use super::theme;

// Grid constants from .superpowers/sdd/design-system.md (relative to the
// 680x456 page frame): PAD=32, field column starts at 232, full-width
// content (section headers, hint text, multi-line boxes) spans 32..648.
const FULL_X: i32 = 32;
const FULL_W: i32 = 616;
const FIELD_X: i32 = 232;

pub struct TaskPage {
    // Kept alive only so lbl_exists' bold font isn't freed; never read again.
    #[allow(dead_code)]
    status_font: nwg::Font,
    // Section-header font (Segoe UI Semibold 15px), shared by both headers
    // below; never read again.
    #[allow(dead_code)]
    hdr_font: nwg::Font,
    // Pure caption/header labels -- Label's Drop destroys its window, so
    // these must outlive build() even though nothing reads them back. This
    // is the control-lifetime bug the original file shipped with.
    #[allow(dead_code)]
    lbl_task_name_caption: nwg::Label,
    #[allow(dead_code)]
    lbl_existing_hdr: nwg::Label,
    #[allow(dead_code)]
    lbl_preview_hdr: nwg::Label,

    pub txt_task_name: nwg::TextInput,
    pub btn_reset_auto: nwg::Button,
    pub lbl_exists: nwg::Label,
    pub txt_existing: nwg::TextBox,
    pub txt_action_preview: nwg::TextBox,
    pub btn_copy_cmd: nwg::Button,
    // Static caption -- set once at build() and never read back.
    #[allow(dead_code)]
    pub lbl_props: nwg::Label,
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> TaskPage {
    let status_font = theme::semibold(14);
    let hdr_font = theme::semibold(15);

    // Row 1 (y=32): task name label + input, "Reset to Auto" button sharing
    // the field column with it.
    let mut lbl_task_name_caption = nwg::Label::default();
    nwg::Label::builder()
        .text("Task name:")
        .position((FULL_X, 30))
        .size((190, 20))
        .parent(parent)
        .build(&mut lbl_task_name_caption)
        .unwrap();
    let mut txt_task_name = nwg::TextInput::default();
    nwg::TextInput::builder()
        .position((FIELD_X, 32))
        .size((258, 26))
        .parent(parent)
        .build(&mut txt_task_name)
        .unwrap();
    let mut btn_reset_auto = nwg::Button::default();
    nwg::Button::builder()
        .text("Reset to Auto")
        .position((500, 29))
        .size((140, 30))
        .parent(parent)
        .build(&mut btn_reset_auto)
        .unwrap();

    // Row 2 (y=72, one row-pitch below row 1): dynamic exists/new-task
    // status line -- bold, full width.
    let mut lbl_exists = nwg::Label::default();
    nwg::Label::builder()
        .text("")
        .position((FULL_X, 72))
        .size((FULL_W, 22))
        .font(Some(&status_font))
        .parent(parent)
        .build(&mut lbl_exists)
        .unwrap();

    // Section: existing task details (16px above, 8px below the header).
    let mut lbl_existing_hdr = nwg::Label::default();
    nwg::Label::builder()
        .text("Existing task details:")
        .position((FULL_X, 110))
        .size((FULL_W, 22))
        .font(Some(&hdr_font))
        .parent(parent)
        .build(&mut lbl_existing_hdr)
        .unwrap();
    let mut txt_existing = nwg::TextBox::default();
    nwg::TextBox::builder()
        .position((FULL_X, 140))
        .size((FULL_W, 90))
        .readonly(true)
        .parent(parent)
        .build(&mut txt_existing)
        .unwrap();

    // Section: action that will be registered (16px above, 8px below).
    let mut lbl_preview_hdr = nwg::Label::default();
    nwg::Label::builder()
        .text("Action that will be registered:")
        .position((FULL_X, 246))
        .size((FULL_W, 22))
        .font(Some(&hdr_font))
        .parent(parent)
        .build(&mut lbl_preview_hdr)
        .unwrap();
    let mut txt_action_preview = nwg::TextBox::default();
    nwg::TextBox::builder()
        .position((FULL_X, 276))
        .size((FULL_W, 70))
        .readonly(true)
        .parent(parent)
        .build(&mut txt_action_preview)
        .unwrap();

    let mut btn_copy_cmd = nwg::Button::default();
    nwg::Button::builder()
        .text("Copy Command")
        .position((FULL_X, 356))
        .size((150, 30))
        .parent(parent)
        .build(&mut btn_copy_cmd)
        .unwrap();

    // Hint line (8px under the copy-command row): static task properties,
    // muted gray, full width, generously tall so the longest wrap never
    // clips.
    let mut lbl_props = nwg::Label::default();
    nwg::Label::builder()
        .text("Runs as SYSTEM \u{b7} At startup \u{b7} Restart 1 min \u{d7}999 \u{b7} Ignore new instances \u{b7} No time limit")
        .position((FULL_X, 394))
        .size((FULL_W, 56))
        .parent(parent)
        .build(&mut lbl_props)
        .unwrap();
    theme::register_muted(&lbl_props.handle);

    TaskPage {
        status_font,
        hdr_font,
        lbl_task_name_caption,
        lbl_existing_hdr,
        lbl_preview_hdr,
        txt_task_name,
        btn_reset_auto,
        lbl_exists,
        txt_existing,
        txt_action_preview,
        btn_copy_cmd,
        lbl_props,
    }
}

impl TaskPage {
    pub fn load(&self, cfg: &Config) {
        self.txt_task_name.set_text(&cfg.task_name);

        let name = crate::task::sanitize_task_name(&cfg.task_name);
        if crate::task::exists(&name) {
            self.lbl_exists.set_text("Task exists - it will be UPDATED.");
            let st = crate::task::query_status(&name);
            self.txt_existing.set_text(&format!(
                "State: {}\r\nLastRunTime: {}\r\nLastRunResult: {}\r\nNextRunTime: {}",
                st.state, st.last_run_time, st.last_run_result, st.next_run_time
            ));
            self.txt_existing.set_visible(true);
        } else {
            self.lbl_exists.set_text("New task will be created.");
            self.txt_existing.set_text("");
            self.txt_existing.set_visible(false);
        }

        let exe = crate::paths::exe_path().display().to_string();
        let config_path = crate::paths::config_path().display().to_string();
        let workdir = crate::paths::install_dir().display().to_string();
        self.txt_action_preview.set_text(&format!(
            "EXE: {exe}\r\nArguments: --monitor --config \"{config_path}\"\r\nWork Dir: {workdir}"
        ));
    }

    /// Always succeeds -- Task never blocks navigation. Returns `bool` to
    /// match the page contract every page shares (only Notify's `save` ever
    /// returns `false`, on invalid email settings).
    pub fn save(&self, cfg: &mut Config) -> bool {
        cfg.task_name = crate::task::sanitize_task_name(&self.txt_task_name.text());
        true
    }

    /// Wired to btn_reset_auto's OnButtonClick.
    pub fn reset_to_auto(&self, state: &super::WizardState) {
        let target = state.cfg.borrow().target_name.clone();
        self.txt_task_name.set_text(&crate::task::auto_task_name(&target));
    }

    /// Wired to btn_copy_cmd's OnButtonClick.
    pub fn copy_command(&self) {
        nwg::Clipboard::set_data_text(&self.btn_copy_cmd, &self.txt_action_preview.text());
    }
}

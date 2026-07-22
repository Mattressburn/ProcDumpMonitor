use crate::config::Config;
use native_windows_gui as nwg;
use std::rc::Rc;

pub struct TaskPage {
    // Kept alive only so lbl_exists' bold font isn't freed; never read again.
    #[allow(dead_code)]
    bold_font: nwg::Font,
    // Pure caption labels -- Label's Drop destroys its window, so these must
    // outlive build() even though nothing reads them back.
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
    let mut bold_font = nwg::Font::default();
    let _ = nwg::Font::builder().family("Segoe UI").weight(700).build(&mut bold_font);

    let mut lbl_task_name_caption = nwg::Label::default();
    nwg::Label::builder()
        .text("Task name:")
        .position((10, 12))
        .size((110, 22))
        .parent(parent)
        .build(&mut lbl_task_name_caption)
        .unwrap();
    let mut txt_task_name = nwg::TextInput::default();
    nwg::TextInput::builder()
        .position((130, 10))
        .size((300, 24))
        .parent(parent)
        .build(&mut txt_task_name)
        .unwrap();
    let mut btn_reset_auto = nwg::Button::default();
    nwg::Button::builder()
        .text("Reset to Auto")
        .position((450, 8))
        .size((130, 26))
        .parent(parent)
        .build(&mut btn_reset_auto)
        .unwrap();

    let mut lbl_exists = nwg::Label::default();
    nwg::Label::builder()
        .text("")
        .position((10, 50))
        .size((540, 22))
        .font(Some(&bold_font))
        .parent(parent)
        .build(&mut lbl_exists)
        .unwrap();

    let mut lbl_existing_hdr = nwg::Label::default();
    nwg::Label::builder()
        .text("Existing task details:")
        .position((10, 76))
        .size((540, 18))
        .parent(parent)
        .build(&mut lbl_existing_hdr)
        .unwrap();
    let mut txt_existing = nwg::TextBox::default();
    nwg::TextBox::builder()
        .position((10, 96))
        .size((700, 90))
        .readonly(true)
        .parent(parent)
        .build(&mut txt_existing)
        .unwrap();

    let mut lbl_preview_hdr = nwg::Label::default();
    nwg::Label::builder()
        .text("Action that will be registered:")
        .position((10, 198))
        .size((540, 18))
        .parent(parent)
        .build(&mut lbl_preview_hdr)
        .unwrap();
    let mut txt_action_preview = nwg::TextBox::default();
    nwg::TextBox::builder()
        .position((10, 218))
        .size((700, 70))
        .readonly(true)
        .parent(parent)
        .build(&mut txt_action_preview)
        .unwrap();

    let mut btn_copy_cmd = nwg::Button::default();
    nwg::Button::builder()
        .text("Copy Command")
        .position((10, 296))
        .size((140, 28))
        .parent(parent)
        .build(&mut btn_copy_cmd)
        .unwrap();

    let mut lbl_props = nwg::Label::default();
    nwg::Label::builder()
        .text("Runs as SYSTEM \u{b7} At startup \u{b7} Restart 1 min \u{d7}999 \u{b7} Ignore new instances \u{b7} No time limit")
        .position((10, 336))
        .size((700, 40))
        .parent(parent)
        .build(&mut lbl_props)
        .unwrap();

    TaskPage {
        bold_font,
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

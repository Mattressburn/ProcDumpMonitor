use crate::config::{Config, TargetType};
use native_windows_gui as nwg;
use std::process::Command;
use std::rc::Rc;

pub struct ReviewPage {
    // Kept alive only so lbl_banner's bold font isn't freed -- Font's Drop
    // destroys the HFONT, which would silently un-bold the label even
    // though construction still succeeds (see page_task.rs's identical
    // note; ALIVE checks don't catch this class of bug).
    #[allow(dead_code)]
    bold_font: nwg::Font,
    pub txt_summary: nwg::TextBox,
    pub btn_create: nwg::Button,
    pub btn_run: nwg::Button,
    pub btn_stop: nwg::Button,
    pub btn_remove: nwg::Button,
    pub btn_save_only: nwg::Button,
    pub btn_open_dumps: nwg::Button,
    pub btn_view_logs: nwg::Button,
    pub btn_copy_args: nwg::Button,
    pub btn_taskschd: nwg::Button,
    pub lbl_banner: nwg::Label,
    pub lst_log: nwg::ListBox<String>,
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> ReviewPage {
    let mut bold_font = nwg::Font::default();
    let _ = nwg::Font::builder().family("Segoe UI").weight(700).build(&mut bold_font);

    let mut txt_summary = nwg::TextBox::default();
    nwg::TextBox::builder()
        .position((10, 8))
        .size((740, 180))
        .readonly(true)
        .parent(parent)
        .build(&mut txt_summary)
        .unwrap();

    let mut btn_create = nwg::Button::default();
    nwg::Button::builder()
        .text("Create Task")
        .position((10, 198))
        .size((110, 28))
        .parent(parent)
        .build(&mut btn_create)
        .unwrap();
    let mut btn_run = nwg::Button::default();
    nwg::Button::builder()
        .text("Run Task Now")
        .position((130, 198))
        .size((110, 28))
        .parent(parent)
        .build(&mut btn_run)
        .unwrap();
    let mut btn_stop = nwg::Button::default();
    nwg::Button::builder()
        .text("Stop Task")
        .position((250, 198))
        .size((110, 28))
        .parent(parent)
        .build(&mut btn_stop)
        .unwrap();
    let mut btn_remove = nwg::Button::default();
    nwg::Button::builder()
        .text("Remove Task")
        .position((370, 198))
        .size((110, 28))
        .parent(parent)
        .build(&mut btn_remove)
        .unwrap();

    let mut btn_save_only = nwg::Button::default();
    nwg::Button::builder()
        .text("Save Config Only")
        .position((10, 234))
        .size((130, 28))
        .parent(parent)
        .build(&mut btn_save_only)
        .unwrap();
    let mut btn_open_dumps = nwg::Button::default();
    nwg::Button::builder()
        .text("Open Dump Folder")
        .position((150, 234))
        .size((130, 28))
        .parent(parent)
        .build(&mut btn_open_dumps)
        .unwrap();
    let mut btn_view_logs = nwg::Button::default();
    nwg::Button::builder()
        .text("View Logs")
        .position((290, 234))
        .size((100, 28))
        .parent(parent)
        .build(&mut btn_view_logs)
        .unwrap();
    let mut btn_copy_args = nwg::Button::default();
    nwg::Button::builder()
        .text("Copy ProcDump Cmd")
        .position((400, 234))
        .size((150, 28))
        .parent(parent)
        .build(&mut btn_copy_args)
        .unwrap();
    let mut btn_taskschd = nwg::Button::default();
    nwg::Button::builder()
        .text("Open Task Scheduler")
        .position((560, 234))
        .size((170, 28))
        .parent(parent)
        .build(&mut btn_taskschd)
        .unwrap();

    let mut lbl_banner = nwg::Label::default();
    nwg::Label::builder()
        .text("")
        .position((10, 272))
        .size((740, 22))
        .font(Some(&bold_font))
        .parent(parent)
        .build(&mut lbl_banner)
        .unwrap();

    let mut lst_log = nwg::ListBox::default();
    nwg::ListBox::builder().position((10, 300)).size((740, 160)).parent(parent).build(&mut lst_log).unwrap();

    ReviewPage {
        bold_font,
        txt_summary,
        btn_create,
        btn_run,
        btn_stop,
        btn_remove,
        btn_save_only,
        btn_open_dumps,
        btn_view_logs,
        btn_copy_args,
        btn_taskschd,
        lbl_banner,
        lst_log,
    }
}

fn build_summary(cfg: &Config) -> String {
    let target_type = match cfg.target_type {
        TargetType::Process => "Process",
        TargetType::Service => "Service",
    };
    let email = if cfg.email_enabled {
        format!("On -> {}", cfg.to_address)
    } else {
        "Off".to_string()
    };
    let webhook = if cfg.webhook_enabled { "On" } else { "Off" };
    format!(
        "Target:            {} ({target_type})\r\n\
         Scenario:          {}\r\n\
         Effective args:    {}\r\n\
         Dump directory:    {}\r\n\
         Task name:         {}\r\n\
         Email:             {email}\r\n\
         Webhook:           {webhook}\r\n\
         Retention:         {} day(s), {} GB max",
        cfg.target_name,
        if cfg.scenario.is_empty() { "Custom" } else { &cfg.scenario },
        crate::procdump::build_args(cfg),
        cfg.dump_directory,
        cfg.task_name,
        cfg.dump_retention_days,
        cfg.dump_retention_max_gb,
    )
}

/// Shells out to this same exe's own CLI verb rather than duplicating
/// task/process logic in the GUI -- one code path, exercised headlessly by
/// Task 8's CLI tests, reused here instead of re-implemented.
fn run_own_verb(verb: &str) -> (bool, String) {
    let out = Command::new(crate::paths::exe_path())
        .args([verb, "--config", &crate::paths::config_path().display().to_string()])
        .output();
    match out {
        Ok(o) => {
            let text =
                format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            (o.status.success(), text.trim().to_string())
        }
        Err(e) => (false, format!("cannot run {verb}: {e}")),
    }
}

impl ReviewPage {
    fn task_button_label(cfg: &Config) -> &'static str {
        if crate::task::exists(&crate::task::sanitize_task_name(&cfg.task_name)) {
            "Update Task"
        } else {
            "Create Task"
        }
    }

    /// Re-renders the summary text and the Create/Update Task button label
    /// from the current `state.cfg` -- shared by `load()` (page entry) and
    /// by every action that can change what's on disk or in Task Scheduler
    /// (Create/Update, Remove), so the page never shows stale values after
    /// an action completes.
    fn refresh_from_state(&self, state: &super::WizardState) {
        let cfg = state.cfg.borrow();
        self.txt_summary.set_text(&build_summary(&cfg));
        self.btn_create.set_text(Self::task_button_label(&cfg));
    }

    pub fn load(&self, cfg: &Config) {
        self.txt_summary.set_text(&build_summary(cfg));
        self.btn_create.set_text(Self::task_button_label(cfg));
        self.lbl_banner.set_text("");
    }

    /// Review has no editable fields of its own; present for the same
    /// build/load/save contract every other page implements.
    pub fn save(&self, _cfg: &mut Config) -> bool {
        true
    }

    /// Sets the banner ("OK: "/"ERROR: " prefix, not color) and appends a
    /// timestamped line to the session log -- every action funnels through
    /// this single spot instead of each button hand-rolling both.
    fn record(&self, ok: bool, detail: &str) {
        let prefix = if ok { "OK:" } else { "ERROR:" };
        self.lbl_banner.set_text(&format!("{prefix} {detail}"));
        let ts = chrono::Local::now().format("%H:%M:%S");
        self.lst_log.push(format!("[{ts}] {prefix} {detail}"));
    }

    /// Wired to btn_create's OnButtonClick. Caller (gui::run) has already
    /// run every other page's `save()` into `state.cfg` first -- same as a
    /// forward wizard nav would -- so this only needs to persist and install.
    pub fn create_task(&self, state: &super::WizardState) {
        let path = crate::paths::config_path();
        if let Err(e) = state.cfg.borrow_mut().save(&path) {
            self.record(false, &format!("Failed to save config: {e}"));
            return;
        }
        let (ok, msg) = run_own_verb("install");
        self.record(ok, &msg);
        self.refresh_from_state(state);
    }

    /// Wired to btn_run's OnButtonClick.
    pub fn run_task(&self) {
        let (ok, msg) = run_own_verb("start");
        self.record(ok, &msg);
    }

    /// Wired to btn_stop's OnButtonClick.
    pub fn stop_task(&self) {
        let (ok, msg) = run_own_verb("stop");
        self.record(ok, &msg);
    }

    /// Wired to btn_remove's OnButtonClick.
    pub fn remove_task(&self, state: &super::WizardState) {
        let (ok, msg) = run_own_verb("uninstall");
        self.record(ok, &msg);
        self.refresh_from_state(state);
    }

    /// Wired to btn_save_only's OnButtonClick.
    pub fn save_config_only(&self, state: &super::WizardState) {
        let path = crate::paths::config_path();
        match state.cfg.borrow_mut().save(&path) {
            Ok(()) => self.record(true, &format!("Config saved to {}.", path.display())),
            Err(e) => self.record(false, &format!("Failed to save config: {e}")),
        }
    }

    /// Wired to btn_open_dumps's OnButtonClick.
    pub fn open_dump_folder(&self, state: &super::WizardState) {
        let dir = state.cfg.borrow().dump_directory.clone();
        match Command::new("explorer.exe").arg(&dir).spawn() {
            Ok(_) => self.record(true, "Opened dump folder."),
            Err(e) => self.record(false, &format!("Could not open dump folder: {e}")),
        }
    }

    /// Wired to btn_view_logs's OnButtonClick.
    pub fn view_logs(&self) {
        match Command::new("notepad.exe").arg(crate::paths::log_path()).spawn() {
            Ok(_) => self.record(true, "Opened log file."),
            Err(e) => self.record(false, &format!("Could not open log file: {e}")),
        }
    }

    /// Wired to btn_copy_args's OnButtonClick.
    pub fn copy_args(&self, state: &super::WizardState) {
        let args = crate::procdump::build_args(&state.cfg.borrow());
        nwg::Clipboard::set_data_text(&self.btn_copy_args, &args);
        self.record(true, "Copied ProcDump command to clipboard.");
    }

    /// Wired to btn_taskschd's OnButtonClick.
    pub fn open_task_scheduler(&self) {
        match Command::new("mmc.exe").arg("taskschd.msc").spawn() {
            Ok(_) => self.record(true, "Opened Task Scheduler."),
            Err(e) => self.record(false, &format!("Could not open Task Scheduler: {e}")),
        }
    }
}

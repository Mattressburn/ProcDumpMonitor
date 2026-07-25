use crate::config::{Config, TargetType};
use native_windows_gui as nwg;
use std::process::Command;
use std::rc::Rc;

use super::theme;

pub struct ReviewPage {
    // Kept in the struct for tidy ownership alongside lbl_banner.
    // (nwg::Font has no Drop impl -- the HFONT is never freed -- so this is
    // convention, not a lifetime requirement; see mod.rs's font note.)
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

// Grid constants (logical px, relative to the 680x456 page frame; see
// .superpowers/sdd/design-system.md). PAD=32 / content width 616 match the
// spec's full-width rule for buttons and preview/summary boxes on this
// field-less, action-heavy page -- there's no label/field column split here.
const PAD: i32 = 32;
const CONTENT_W: i32 = 616; // 680 - 2*PAD
const BTN_H: i32 = 30;
const BTN_GAP: i32 = 10;

const SUMMARY_Y: i32 = PAD;
const SUMMARY_H: i32 = 160;
const ROW1_Y: i32 = SUMMARY_Y + SUMMARY_H + 12; // primary task actions row
const ROW2_Y: i32 = ROW1_Y + BTN_H + 12; // secondary actions row
const BANNER_Y: i32 = ROW2_Y + BTN_H + 16;
const BANNER_H: i32 = 22;
const LOG_Y: i32 = BANNER_Y + BANNER_H + 8;
const LOG_H: i32 = 118; // fills to the frame's bottom margin (see design-system.md)

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> ReviewPage {
    // Themed Segoe UI Semibold at body size -- also fixes the original's
    // unspecified-size bold Font, which silently fell back to a default
    // size that didn't match the 15px body font used everywhere else.
    let bold_font = theme::semibold(15);

    // ---- Summary: full width, top (design-system.md "Summary area
    // full-width at top") -----------------------------------------------
    let mut txt_summary = nwg::TextBox::default();
    nwg::TextBox::builder()
        .position((PAD, SUMMARY_Y))
        .size((CONTENT_W, SUMMARY_H))
        .readonly(true)
        .parent(parent)
        .build(&mut txt_summary)
        .unwrap();

    // ---- Primary task actions row: Create/Update, Run, Stop, Remove -------
    // x of each button = previous button's x + width + BTN_GAP, so the row
    // stays gap-consistent if a caption's width ever needs to change.
    // Equal width for all four -- 144 comfortably fits the longest caption
    // ("Run Task Now") and keeps the row inside the 648 right margin.
    let (create_w, run_w, stop_w, remove_w) = (144, 144, 144, 144);
    let create_x = PAD;
    let run_x = create_x + create_w + BTN_GAP;
    let stop_x = run_x + run_w + BTN_GAP;
    let remove_x = stop_x + stop_w + BTN_GAP;

    let mut btn_create = nwg::Button::default();
    nwg::Button::builder()
        .text("Create Task")
        .position((create_x, ROW1_Y))
        .size((create_w, BTN_H))
        .parent(parent)
        .build(&mut btn_create)
        .unwrap();
    let mut btn_run = nwg::Button::default();
    nwg::Button::builder()
        .text("Run Task Now")
        .position((run_x, ROW1_Y))
        .size((run_w, BTN_H))
        .parent(parent)
        .build(&mut btn_run)
        .unwrap();
    let mut btn_stop = nwg::Button::default();
    nwg::Button::builder()
        .text("Stop Task")
        .position((stop_x, ROW1_Y))
        .size((stop_w, BTN_H))
        .parent(parent)
        .build(&mut btn_stop)
        .unwrap();
    let mut btn_remove = nwg::Button::default();
    nwg::Button::builder()
        .text("Remove Task")
        .position((remove_x, ROW1_Y))
        .size((remove_w, BTN_H))
        .parent(parent)
        .build(&mut btn_remove)
        .unwrap();

    // ---- Secondary actions row: Save config only, Open dump folder, View
    // logs, Copy args, Task Scheduler. Captions shortened from the original
    // ("Save Config Only" -> "Save Config", "Copy ProcDump Cmd" -> "Copy
    // Args", "Open Task Scheduler" -> "Task Scheduler") -- five buttons must
    // share the page's 616px content width, and design-system.md permits
    // wording changes for layout fit; every width below is sized with
    // headroom over its caption at body-font size (~7px/char), so nothing
    // clips. -------------------------------------------------------------
    // args_w trimmed 110->100 so this row's right edge (638) lands on the
    // same x as row 1's after the row-1 equal-width fix, instead of 10px
    // further right.
    let (save_w, dumps_w, logs_w, args_w) = (110, 128, 110, 100);
    let save_x = PAD;
    let dumps_x = save_x + save_w + BTN_GAP;
    let logs_x = dumps_x + dumps_w + BTN_GAP;
    let args_x = logs_x + logs_w + BTN_GAP;
    let taskschd_x = args_x + args_w + BTN_GAP;

    let mut btn_save_only = nwg::Button::default();
    nwg::Button::builder()
        .text("Save Config")
        .position((save_x, ROW2_Y))
        .size((save_w, BTN_H))
        .parent(parent)
        .build(&mut btn_save_only)
        .unwrap();
    let mut btn_open_dumps = nwg::Button::default();
    nwg::Button::builder()
        .text("Open Dump Folder")
        .position((dumps_x, ROW2_Y))
        .size((dumps_w, BTN_H))
        .parent(parent)
        .build(&mut btn_open_dumps)
        .unwrap();
    let mut btn_view_logs = nwg::Button::default();
    nwg::Button::builder()
        .text("View Logs")
        .position((logs_x, ROW2_Y))
        .size((logs_w, BTN_H))
        .parent(parent)
        .build(&mut btn_view_logs)
        .unwrap();
    let mut btn_copy_args = nwg::Button::default();
    nwg::Button::builder()
        .text("Copy Args")
        .position((args_x, ROW2_Y))
        .size((args_w, BTN_H))
        .parent(parent)
        .build(&mut btn_copy_args)
        .unwrap();
    let mut btn_taskschd = nwg::Button::default();
    nwg::Button::builder()
        .text("Task Scheduler")
        .position((taskschd_x, ROW2_Y))
        .size((118, BTN_H))
        .parent(parent)
        .build(&mut btn_taskschd)
        .unwrap();

    // ---- Status banner + activity log (full width) ------------------------
    let mut lbl_banner = nwg::Label::default();
    nwg::Label::builder()
        .text("")
        .position((PAD, BANNER_Y))
        .size((CONTENT_W, BANNER_H))
        .font(Some(&bold_font))
        .parent(parent)
        .build(&mut lbl_banner)
        .unwrap();

    let mut lst_log = nwg::ListBox::default();
    nwg::ListBox::builder()
        .position((PAD, LOG_Y))
        .size((CONTENT_W, LOG_H))
        .parent(parent)
        .build(&mut lst_log)
        .unwrap();

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
        "Target: {} ({target_type})\r\n\
         Scenario: {}\r\n\
         Effective args: {}\r\n\
         Dump directory: {}\r\n\
         Task name: {}\r\n\
         Email: {email}\r\n\
         Webhook: {webhook}\r\n\
         Retention: {} day(s), {} GB max",
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

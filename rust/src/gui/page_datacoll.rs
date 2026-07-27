//! Data Collection page — the PS1's main tab restyled into the sidebar
//! shell, plus the new "LogDump logs, dumps & task state" bundle
//! checkbox and the 7-day event-log default.

use crate::collect::{self, datacoll, discover, pdm_bundle};
use crate::{paths, task};
use native_windows_gui as nwg;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::collect_runner::{self, CollectRunner};
use super::theme;

pub struct DataCollPage {
    #[allow(dead_code)]
    header_font: nwg::Font,
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    pub txt_save: nwg::TextInput,
    pub btn_browse: nwg::Button,

    pub chk_sysinfo: nwg::CheckBox,
    pub chk_apps: nwg::CheckBox,
    pub chk_updates: nwg::CheckBox,
    pub chk_history: nwg::CheckBox,
    pub chk_evtx: nwg::CheckBox,
    pub chk_evtx_full: nwg::CheckBox,
    pub chk_bulk: nwg::CheckBox,
    pub chk_swh: nwg::CheckBox,
    pub chk_pdm: nwg::CheckBox,

    /// Parallel to discover::LOG_COMPONENTS.
    pub chk_components: Vec<nwg::CheckBox>,
    pub btn_all: nwg::Button,
    pub btn_none: nwg::Button,

    pub btn_start: nwg::Button,
    pub btn_open: nwg::Button,
    pub lbl_status: nwg::Label,
    pub lst_progress: nwg::ListBox<String>,
    pub runner: CollectRunner,
}

fn mk_label<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    text: &str,
    pos: (i32, i32),
    size: (i32, i32),
) -> nwg::Label {
    let mut l = nwg::Label::default();
    nwg::Label::builder().text(text).position(pos).size(size).parent(parent).build(&mut l).unwrap();
    l
}

fn mk_check<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    text: &str,
    pos: (i32, i32),
    size: (i32, i32),
    checked: bool,
) -> nwg::CheckBox {
    let mut c = nwg::CheckBox::default();
    nwg::CheckBox::builder()
        .text(text)
        .position(pos)
        .size(size)
        .check_state(if checked { nwg::CheckBoxState::Checked } else { nwg::CheckBoxState::Unchecked })
        .parent(parent)
        .build(&mut c)
        .unwrap();
    c
}

fn mk_button<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    text: &str,
    pos: (i32, i32),
    size: (i32, i32),
) -> nwg::Button {
    let mut b = nwg::Button::default();
    nwg::Button::builder().text(text).position(pos).size(size).parent(parent).build(&mut b).unwrap();
    b
}

fn checked(c: &nwg::CheckBox) -> bool {
    c.check_state() == nwg::CheckBoxState::Checked
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> DataCollPage {
    let header_font = theme::semibold(15);
    let mut captions: Vec<nwg::Label> = Vec::new();

    const PAD: i32 = 32;
    const COL2: i32 = 340;

    captions.push(mk_label(parent, "Save path (blank = Desktop):", (PAD, 14), (220, 20)));
    let txt_save = super_mk_text(parent, (260, 12), (278, 26));
    let btn_browse = mk_button(parent, "Browse...", (548, 10), (100, 30));

    captions.push({
        let l = mk_label(parent, "Extra collections", (PAD, 50), (300, 20));
        l.set_font(Some(&header_font));
        l
    });
    let chk_sysinfo = mk_check(parent, "System information", (PAD, 76), (280, 22), true);
    let chk_apps = mk_check(parent, "Installed applications (all)", (COL2, 76), (280, 22), true);
    let chk_updates = mk_check(parent, "Installed updates (if any)", (PAD, 102), (280, 22), true);
    let chk_history = mk_check(parent, "InstallHistory.xml", (COL2, 102), (280, 22), true);
    let chk_evtx =
        mk_check(parent, "Event logs (Application + System)", (PAD, 128), (280, 22), true);
    let chk_evtx_full = mk_check(parent, "Full export (default: last 7 days)", (COL2, 128), (280, 22), false);
    let chk_bulk = mk_check(parent, "Bulk updates (if any)", (PAD, 154), (280, 22), false);
    let chk_swh = mk_check(parent, "SWHSystem settings", (COL2, 154), (280, 22), true);
    let chk_pdm = mk_check(
        parent,
        "LogDump logs, dumps && task state",
        (PAD, 180),
        (400, 22),
        true,
    );

    captions.push({
        let l = mk_label(parent, "Log components (install-dir based)", (PAD, 212), (400, 20));
        l.set_font(Some(&header_font));
        l
    });
    let mut chk_components: Vec<nwg::CheckBox> = Vec::new();
    for (i, (name, _rel)) in discover::LOG_COMPONENTS.iter().enumerate() {
        let x = if i % 2 == 0 { PAD } else { COL2 };
        let y = 238 + (i as i32 / 2) * 26;
        chk_components.push(mk_check(parent, name, (x, y), (300, 22), false));
    }
    let btn_all = mk_button(parent, "Select all", (PAD, 348), (100, 28));
    let btn_none = mk_button(parent, "Select none", (140, 348), (110, 28));

    let btn_start = mk_button(parent, "Start collection", (PAD, 388), (150, 32));
    let btn_open = mk_button(parent, "Open last output", (192, 388), (150, 32));
    let lbl_status = mk_label(parent, "Status: Idle", (356, 394), (292, 20));
    theme::register_muted(&lbl_status.handle);

    let mut lst_progress = nwg::ListBox::default();
    nwg::ListBox::builder()
        .position((PAD, 430))
        .size((616, 156))
        .parent(parent)
        .build(&mut lst_progress)
        .unwrap();

    let runner = collect_runner::build(parent);

    DataCollPage {
        header_font,
        captions,
        txt_save,
        btn_browse,
        chk_sysinfo,
        chk_apps,
        chk_updates,
        chk_history,
        chk_evtx,
        chk_evtx_full,
        chk_bulk,
        chk_swh,
        chk_pdm,
        chk_components,
        btn_all,
        btn_none,
        btn_start,
        btn_open,
        lbl_status,
        lst_progress,
        runner,
    }
}

fn super_mk_text<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    pos: (i32, i32),
    size: (i32, i32),
) -> nwg::TextInput {
    let mut t = nwg::TextInput::default();
    nwg::TextInput::builder().position(pos).size(size).parent(parent).build(&mut t).unwrap();
    t
}

impl DataCollPage {
    pub fn set_components(&self, all: bool) {
        let st = if all { nwg::CheckBoxState::Checked } else { nwg::CheckBoxState::Unchecked };
        for c in &self.chk_components {
            c.set_check_state(st);
        }
    }

    pub fn browse_save_path(&self, parent: nwg::ControlHandle) {
        let mut dialog = nwg::FileDialog::default();
        let built = nwg::FileDialog::builder()
            .title("Select output folder")
            .action(nwg::FileDialogAction::OpenDirectory)
            .build(&mut dialog)
            .is_ok();
        if built && dialog.run(Some(parent)) {
            if let Ok(p) = dialog.get_selected_item() {
                self.txt_save.set_text(&p.to_string_lossy());
            }
        }
    }

    /// Start the run on a worker thread. Options are captured NOW from the
    /// checkboxes; discovery (registry, path probing) happens on the worker.
    pub fn start(&self, state: &super::WizardState) {
        if self.runner.running.get() {
            return;
        }
        let base = resolve_base(&self.txt_save.text());
        let wanted: Vec<String> = self
            .chk_components
            .iter()
            .zip(discover::LOG_COMPONENTS.iter())
            .filter(|(c, _)| checked(c))
            .map(|(_, (name, _))| name.to_string())
            .collect();

        let pdm = if checked(&self.chk_pdm) {
            let cfg = state.cfg.borrow();
            Some(pdm_bundle::Options {
                log_dir: paths::log_dir(),
                health_path: paths::health_path(),
                config_path: paths::config_path(),
                task_name: task::sanitize_task_name(&cfg.task_name),
                dump_dir: PathBuf::from(&cfg.dump_directory),
                max_dump_bytes: pdm_bundle::DEFAULT_MAX_DUMP_BYTES,
            })
        } else {
            None
        };

        let mut opts = datacoll::Options {
            components: Vec::new(), // resolved on the worker below
            system_info: checked(&self.chk_sysinfo),
            installed_apps: checked(&self.chk_apps),
            installed_updates: checked(&self.chk_updates),
            event_logs: checked(&self.chk_evtx),
            event_logs_full: checked(&self.chk_evtx_full),
            install_history: checked(&self.chk_history),
            bulk_updates: checked(&self.chk_bulk),
            swh_settings: checked(&self.chk_swh),
            pdm_bundle: pdm,
        };

        let started = self.runner.start(move |progress| {
            let exists = |p: &Path| p.exists();
            let loc = discover::install_location();
            let (jci, tyco) = discover::vendor_roots(loc.as_deref(), &exists);
            opts.components = discover::log_component_paths(&jci, &tyco)
                .into_iter()
                .filter(|(name, _)| wanted.iter().any(|w| w == name))
                .collect();

            let mut ctx = collect::RunContext::start(&base, progress)
                .map_err(|e| format!("cannot create run folder: {e}"))?;
            datacoll::run(&mut ctx, &opts);
            Ok(ctx.finish())
        });
        if started {
            self.lst_progress.clear();
            self.lbl_status.set_text("Status: Running...");
            self.btn_start.set_enabled(false);
        }
    }

    /// OnNotice handler: pump progress lines into the list.
    pub fn on_notice(&self) {
        pump(&self.runner, &self.lst_progress, &self.lbl_status, &self.btn_start);
    }
}

/// Shared notice-pump for all three collector pages.
pub fn pump(
    runner: &CollectRunner,
    list: &nwg::ListBox<String>,
    status: &nwg::Label,
    start_btn: &nwg::Button,
) {
    let (lines, finished) = runner.drain();
    for l in lines {
        list.push(l);
        let len = list.len();
        if len > 0 {
            list.set_selection(Some(len - 1)); // keep the newest line visible
        }
    }
    if let Some(result) = finished {
        match result {
            Ok(dir) => status.set_text(&format!("Status: Done \u{2014} {}", dir.display())),
            Err(e) => status.set_text(&format!("Status: FAILED \u{2014} {e}")),
        }
        start_btn.set_enabled(true);
    }
}

/// Blank -> Desktop (PS1 behavior).
pub fn resolve_base(text: &str) -> PathBuf {
    let t = text.trim();
    if t.is_empty() {
        crate::cli::default_collect_base()
    } else {
        PathBuf::from(t)
    }
}

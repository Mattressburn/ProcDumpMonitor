//! The merged Monitor page (approved 2026-07-25 spec, Option A): target +
//! triggers + schedule + notify essentials + live status, one page. The
//! power-user fields moved to dlg_advanced; full SMTP config to dlg_smtp.
//! The primary/secondary action buttons live in the shell footer (mod.rs)
//! and call into this page.

use crate::config::{Config, TargetType};
use crate::procdump::Preset;
use crate::{bitness, health, paths, services, task};
use native_windows_gui as nwg;
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

use super::theme;

const DUMP_TYPES: [&str; 4] = ["Full", "MiniPlus", "Mini", "ThreadDump"];

/// One entry in the combined target dropdown.
#[derive(Clone)]
pub struct TargetEntry {
    pub name: String,
    pub is_service: bool,
}

pub struct MonitorPage {
    #[allow(dead_code)]
    header_font: nwg::Font,
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    // Target
    pub cmb_target: nwg::ComboBox<String>,
    pub btn_refresh: nwg::Button,
    pub chk_show_all: nwg::CheckBox,
    pub entries: RefCell<Vec<TargetEntry>>,
    /// Manual override typed in the Advanced dialog (rare: a process that
    /// isn't running yet). Non-empty wins over the dropdown selection.
    pub manual_target: RefCell<String>,

    // Triggers & output
    pub cmb_scenario: nwg::ComboBox<String>,
    pub lbl_bitness: nwg::Label,
    pub cmb_dump_type: nwg::ComboBox<String>,
    pub chk_exception: nwg::CheckBox,
    pub chk_hang: nwg::CheckBox,
    pub chk_terminate: nwg::CheckBox,
    pub txt_cpu: nwg::TextInput,
    pub txt_cpu_low: nwg::TextInput,
    pub txt_cpu_dur: nwg::TextInput,
    pub txt_count: nwg::TextInput,
    pub txt_mem: nwg::TextInput,
    pub txt_procdump_path: nwg::TextInput,
    pub btn_browse_pd: nwg::Button,
    pub txt_dump_dir: nwg::TextInput,
    pub btn_browse_dir: nwg::Button,
    pub txt_effective: nwg::TextInput,

    // Schedule & notifications
    pub txt_task_name: nwg::TextInput,
    pub btn_advanced: nwg::Button,
    pub chk_email: nwg::CheckBox,
    pub txt_to: nwg::TextInput,
    pub btn_smtp: nwg::Button,
    pub chk_webhook: nwg::CheckBox,
    pub txt_webhook: nwg::TextInput,
    pub chk_autocollect: nwg::CheckBox,

    // Live status rows
    pub lbl_st_task: nwg::Label,
    pub lbl_st_monitor: nwg::Label,
    pub lbl_st_dumps: nwg::Label,
    pub lbl_st_alert: nwg::Label,
    /// Last footer-action failure, shown on the alert row until the next
    /// action or a task-state change clears it.
    action_error: RefCell<Option<String>>,

    option_handles: Vec<nwg::ControlHandle>,
    suppress_custom: Cell<bool>,
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

fn mk_header<P: Into<nwg::ControlHandle> + Copy>(parent: P, text: &str, y: i32, font: &nwg::Font) -> nwg::Label {
    let mut l = nwg::Label::default();
    nwg::Label::builder()
        .text(text)
        .position((32, y))
        .size((616, 20))
        .font(Some(font))
        .parent(parent)
        .build(&mut l)
        .unwrap();
    l
}

fn mk_text<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    pos: (i32, i32),
    size: (i32, i32),
    readonly: bool,
) -> nwg::TextInput {
    let mut t = nwg::TextInput::default();
    nwg::TextInput::builder()
        .position(pos)
        .size(size)
        .readonly(readonly)
        .parent(parent)
        .build(&mut t)
        .unwrap();
    t
}

fn mk_check<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    text: &str,
    pos: (i32, i32),
    size: (i32, i32),
) -> nwg::CheckBox {
    let mut c = nwg::CheckBox::default();
    nwg::CheckBox::builder().text(text).position(pos).size(size).parent(parent).build(&mut c).unwrap();
    c
}

/// A ComboBox whose dropdown list actually has a scrollbar.
///
/// nwg never passes `WS_VSCROLL`: `ComboBoxFlags` exposes only
/// VISIBLE/DISABLED/TAB_STOP and `forced_flags()` is just
/// `CBS_DROPDOWNLIST | WS_CHILD | WS_BORDER`. Win32 requires WS_VSCROLL **at
/// creation** for a combobox's drop-down list to get a vertical scrollbar, so
/// every stock nwg combo silently caps at the ~30 rows Windows shows by
/// default (CB_GETMINVISIBLE) with no way to reach the rest — verified: no
/// WS_VSCROLL on the internal listbox and real mouse-wheel input left
/// CB_GETTOPINDEX pinned at 0. `from_bits_unchecked` (bitflags 1.3) is the
/// only way to add the bit through nwg's typed builder. Windows draws the bar
/// only when items overflow, so short combos are unaffected.
fn mk_combo<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    pos: (i32, i32),
    size: (i32, i32),
) -> nwg::ComboBox<String> {
    const WS_VSCROLL: u32 = 0x0020_0000;
    let flags = unsafe {
        nwg::ComboBoxFlags::from_bits_unchecked(
            nwg::ComboBoxFlags::VISIBLE.bits()
                | nwg::ComboBoxFlags::TAB_STOP.bits()
                | WS_VSCROLL,
        )
    };
    let mut c = nwg::ComboBox::default();
    nwg::ComboBox::builder()
        .flags(flags)
        .position(pos)
        .size(size)
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

fn set_checked(c: &nwg::CheckBox, v: bool) {
    c.set_check_state(if v { nwg::CheckBoxState::Checked } else { nwg::CheckBoxState::Unchecked });
}

fn parse_i32(t: &nwg::TextInput) -> i32 {
    t.text().trim().parse::<i32>().unwrap_or(0)
}

/// Render the bitness label from an already-made decision. PURE: no env, no
/// filesystem, no registry — split from `bitness_text` so the 32-bit-OS branch
/// (the half of the old bug that hardcoded `os_is_64: true`) is testable on a
/// 64-bit host by handing `select_binary` a `false`, instead of mutating
/// PROCESSOR_ARCHITECTURE under parallel test threads.
///
/// The `Unknown` arm must keep `choice.summary` intact: it is the only place
/// the word "Unknown" appears, and a svchost-hosted 32-bit service genuinely
/// cannot be resolved from a PE. Shortening this to the binary name would read
/// as a verified 64-bit answer.
fn bitness_label(b: bitness::Bitness, source: &str, choice: &bitness::BinaryChoice) -> String {
    match (&choice.warning, b) {
        (Some(w), _) => format!("{} - {w}", choice.summary),
        (None, bitness::Bitness::Unknown) => format!(
            "{} - could not determine target bitness; verify manually.",
            choice.summary
        ),
        (None, _) => format!("{} (via {source})", choice.summary),
    }
}

/// The label text for `cfg` — the monitor's own `resolve` + `os_is_64`, so the
/// preview cannot disagree with what the scheduled task will do.
///
/// The empty-target guard sits BEFORE `resolve` on purpose: with an empty name
/// and `TargetType::Service`, `resolve` would spawn `reg.exe` against the bare
/// `...\Services` key. Callers are `load()` (startup + every sidebar switch
/// onto Monitor) and `on_target_picked()` — user-paced. Do NOT call this from
/// `refresh_status` (3s timer) or `write_fields` (per keystroke): for a Service
/// target `resolve` costs one `reg.exe` spawn.
fn bitness_text(cfg: &Config, procdump_path: &str) -> String {
    if cfg.target_name.trim().is_empty() {
        return String::new();
    }
    let pd_dir = std::path::Path::new(procdump_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(paths::install_dir);
    let (b, source) = bitness::resolve(cfg);
    bitness_label(b, source, &bitness::select_binary(b, &pd_dir, bitness::os_is_64()))
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> MonitorPage {
    let header_font = theme::semibold(15);
    let mut captions: Vec<nwg::Label> = Vec::new();

    const PAD: i32 = 32;
    const FULL_W: i32 = 616;
    const FIELD_X: i32 = 232;
    const FIELD_H: i32 = 26;

    // ---- Target ---------------------------------------------------------
    captions.push(mk_header(parent, "Target", 12, &header_font));

    captions.push(mk_label(parent, "Process or service:", (PAD, 38), (190, 20)));
    // Height stays on the design-system field height: measured, the create
    // height does NOT control this dropdown's list height (Windows sizes it to
    // CB_GETMINVISIBLE = 30 rows either way -- 26 and 300 both gave a 572px
    // list). Reaching the rest of the items is `mk_combo`'s WS_VSCROLL, not
    // this number.
    let cmb_target = mk_combo(parent, (FIELD_X, 40), (308, FIELD_H));
    let btn_refresh = mk_button(parent, "Refresh", (548, 38), (100, 30));
    let chk_show_all = mk_check(parent, "Include stopped services", (FIELD_X, 74), (330, 22));

    // ---- Dump triggers & output -----------------------------------------
    captions.push(mk_header(parent, "Dump triggers & output", 104, &header_font));

    captions.push(mk_label(parent, "Scenario:", (PAD, 130), (190, 20)));
    let cmb_scenario = mk_combo(parent, (FIELD_X, 132), (300, FIELD_H));
    {
        let mut names: Vec<String> = Preset::all().iter().map(|p| p.name.to_string()).collect();
        names.push("Custom".into());
        cmb_scenario.set_collection(names);
    }
    let lbl_bitness = mk_label(parent, "", (PAD, 162), (FULL_W, 16));
    theme::register_muted(&lbl_bitness.handle);

    captions.push(mk_label(parent, "Dump type:", (PAD, 182), (190, 20)));
    let cmb_dump_type = mk_combo(parent, (FIELD_X, 184), (95, FIELD_H));
    cmb_dump_type.set_collection(DUMP_TYPES.iter().map(|s| s.to_string()).collect());
    let chk_exception = mk_check(parent, "-e unhandled exception", (FIELD_X + 103, 184), (180, 22));
    let chk_hang = mk_check(parent, "-h hung window", (FIELD_X + 291, 184), (118, 22));

    captions.push(mk_label(parent, "CPU% / Low% / Dur / Max:", (PAD, 214), (190, 20)));
    let txt_cpu = mk_text(parent, (FIELD_X, 216), (40, FIELD_H), false);
    let txt_cpu_low = mk_text(parent, (FIELD_X + 46, 216), (40, FIELD_H), false);
    let txt_cpu_dur = mk_text(parent, (FIELD_X + 92, 216), (40, FIELD_H), false);
    let txt_count = mk_text(parent, (FIELD_X + 138, 216), (40, FIELD_H), false);
    let chk_terminate = mk_check(parent, "-t on terminate", (FIELD_X + 184, 216), (118, 22));
    captions.push(mk_label(parent, "MB (-m):", (FIELD_X + 308, 214), (56, 20)));
    let txt_mem = mk_text(parent, (FIELD_X + 368, 216), (44, FIELD_H), false);

    captions.push(mk_label(parent, "ProcDump path:", (PAD, 248), (190, 20)));
    let txt_procdump_path = mk_text(parent, (FIELD_X, 250), (290, FIELD_H), false);
    let btn_browse_pd = mk_button(parent, "Browse...", (FIELD_X + 298, 248), (110, 30));

    captions.push(mk_label(parent, "Dump directory:", (PAD, 282), (190, 20)));
    let txt_dump_dir = mk_text(parent, (FIELD_X, 284), (290, FIELD_H), false);
    let btn_browse_dir = mk_button(parent, "Browse...", (FIELD_X + 298, 282), (110, 30));

    let txt_effective = mk_text(parent, (PAD, 316), (FULL_W, FIELD_H), true);

    // ---- Schedule & notifications ----------------------------------------
    captions.push(mk_header(parent, "Schedule & notifications", 352, &header_font));

    captions.push(mk_label(parent, "Task name:", (PAD, 378), (190, 20)));
    let txt_task_name = mk_text(parent, (FIELD_X, 380), (258, FIELD_H), false);
    let btn_advanced = mk_button(parent, "Advanced...", (500, 378), (140, 30));

    captions.push(mk_label(parent, "Notifications:", (PAD, 412), (190, 20)));
    let chk_email = mk_check(parent, "Email", (FIELD_X, 414), (64, 22));
    let txt_to = mk_text(parent, (FIELD_X + 70, 414), (238, FIELD_H), false);
    txt_to.set_placeholder_text(Some("to@example.com (; separated)"));
    let btn_smtp = mk_button(parent, "SMTP...", (548, 412), (92, 30));

    let chk_webhook = mk_check(parent, "Webhook", (FIELD_X, 448), (84, 22));
    let txt_webhook = mk_text(parent, (FIELD_X + 90, 448), (318, FIELD_H), false);

    let chk_autocollect = mk_check(
        parent,
        "Auto-collect a support bundle when a dump is captured",
        (FIELD_X, 478),
        (408, 22),
    );

    // ---- Live status (fits inside the 596-tall frame: last row ends at
    // 578+15=593) ------------------------------------------------------------
    captions.push(mk_header(parent, "Monitor status", 502, &header_font));
    let lbl_st_task = mk_label(parent, "", (PAD, 524), (FULL_W, 16));
    let lbl_st_monitor = mk_label(parent, "", (PAD, 542), (FULL_W, 16));
    let lbl_st_dumps = mk_label(parent, "", (PAD, 560), (FULL_W, 16));
    let lbl_st_alert = mk_label(parent, "", (PAD, 578), (FULL_W, 15));

    let option_handles = vec![
        cmb_dump_type.handle,
        chk_exception.handle,
        chk_hang.handle,
        chk_terminate.handle,
        txt_cpu.handle,
        txt_cpu_low.handle,
        txt_cpu_dur.handle,
        txt_count.handle,
        txt_mem.handle,
        txt_procdump_path.handle,
        txt_dump_dir.handle,
    ];

    let page = MonitorPage {
        header_font,
        captions,
        cmb_target,
        btn_refresh,
        chk_show_all,
        entries: RefCell::new(Vec::new()),
        manual_target: RefCell::new(String::new()),
        cmb_scenario,
        lbl_bitness,
        cmb_dump_type,
        chk_exception,
        chk_hang,
        chk_terminate,
        txt_cpu,
        txt_cpu_low,
        txt_cpu_dur,
        txt_count,
        txt_mem,
        txt_procdump_path,
        btn_browse_pd,
        txt_dump_dir,
        btn_browse_dir,
        txt_effective,
        txt_task_name,
        btn_advanced,
        chk_email,
        txt_to,
        btn_smtp,
        chk_webhook,
        txt_webhook,
        chk_autocollect,
        lbl_st_task,
        lbl_st_monitor,
        lbl_st_dumps,
        lbl_st_alert,
        action_error: RefCell::new(None),
        option_handles,
        suppress_custom: Cell::new(false),
    };
    page.refresh_targets();
    page
}

impl MonitorPage {
    // ---- Target dropdown --------------------------------------------------

    /// Rebuild the combined target list. PROCESSES FIRST (they're the common
    /// case and would otherwise be buried under 150+ services), then running
    /// services; "Include stopped services" appends the stopped ones. Keeps
    /// the current selection by name when it survives the refresh.
    pub fn refresh_targets(&self) {
        let show_stopped = checked(&self.chk_show_all);
        let selected = self.selected_entry().map(|e| e.name.clone());

        let mut entries: Vec<TargetEntry> = Vec::new();
        let mut labels: Vec<String> = Vec::new();

        // Running processes (Toolhelp only enumerates live ones). Skip the
        // PID-0 pseudo-process: "[System Process]" isn't a real image name and
        // can never be dumped, but sorts first and looks like the default pick.
        for p in bitness::list_process_names() {
            if p.starts_with('[') {
                continue;
            }
            labels.push(format!("Proc: {p}"));
            entries.push(TargetEntry { name: p, is_service: false });
        }
        // Services, running first so the useful ones stay near the top.
        let all_services = services::list();
        for s in all_services.iter().filter(|s| s.running) {
            labels.push(format!("Svc: {} ({})", s.display, s.name));
            entries.push(TargetEntry { name: s.name.clone(), is_service: true });
        }
        if show_stopped {
            for s in all_services.iter().filter(|s| !s.running) {
                labels.push(format!("Svc: {} ({}) - stopped", s.display, s.name));
                entries.push(TargetEntry { name: s.name.clone(), is_service: true });
            }
        }
        self.cmb_target.set_collection(labels);
        if let Some(sel) = selected {
            if let Some(i) = entries.iter().position(|e| e.name.eq_ignore_ascii_case(&sel)) {
                self.cmb_target.set_selection(Some(i));
            }
        }
        *self.entries.borrow_mut() = entries;
    }

    fn selected_entry(&self) -> Option<TargetEntry> {
        self.cmb_target
            .selection()
            .and_then(|i| self.entries.borrow().get(i).cloned())
    }

    /// Effective target (name, type): manual override beats the dropdown.
    fn effective_target(&self) -> (String, TargetType) {
        let manual = self.manual_target.borrow().trim().to_string();
        let picked = self.selected_entry();
        if !manual.is_empty() {
            let picked_svc = picked.as_ref().filter(|e| e.is_service).map(|e| e.name.as_str());
            let t = crate::config::infer_target_type(&manual, picked_svc);
            return (manual, t);
        }
        match picked {
            Some(e) => {
                let t = if e.is_service { TargetType::Service } else { TargetType::Process };
                (e.name, t)
            }
            None => (String::new(), TargetType::Process),
        }
    }

    /// Wired to cmb_target's OnComboxBoxSelection: picking from the list
    /// clears any manual override and refreshes bitness + preview. Also syncs
    /// the auto task-name box live (save() no longer runs during preview).
    pub fn on_target_picked(&self, state: &super::WizardState) {
        self.manual_target.borrow_mut().clear();
        let (name, _) = self.effective_target();
        let prev_target = state.cfg.borrow().target_name.clone();
        let typed_task = task::sanitize_task_name(&self.txt_task_name.text());
        if typed_task == task::auto_task_name(&prev_target)
            || typed_task == "ProcDump Monitor"
            || typed_task.is_empty()
        {
            self.txt_task_name.set_text(&task::auto_task_name(&name));
        }
        // update_bitness needs a &Config. Build it with the CONTROL-PURE
        // write_fields on a throwaway clone -- never save(), which would
        // DPAPI-encrypt the typed webhook URL into the discarded clone and
        // clear the live field, destroying the user's URL.
        let mut probe = state.cfg.borrow().clone();
        self.write_fields(&mut probe);
        self.update_bitness(&probe, &self.txt_procdump_path.text());
        self.refresh_preview(state);
    }

    // ---- Scenario / options ------------------------------------------------

    pub fn is_option_control(&self, h: nwg::ControlHandle) -> bool {
        self.option_handles.contains(&h)
    }

    pub fn on_option_changed(&self, state: &super::WizardState) {
        if self.suppress_custom.get() {
            return;
        }
        self.cmb_scenario.set_selection(Some(Preset::all().len()));
        state.cfg.borrow_mut().scenario = String::new();
        self.refresh_preview(state);
    }

    /// The Advanced dialog edited config fields directly: flip to Custom and
    /// refresh the preview.
    pub fn on_advanced_changed(&self, state: &super::WizardState) {
        self.cmb_scenario.set_selection(Some(Preset::all().len()));
        state.cfg.borrow_mut().scenario = String::new();
        self.refresh_preview(state);
    }

    pub fn on_scenario_selected(&self, state: &super::WizardState) {
        if self.suppress_custom.get() {
            return;
        }
        let Some(i) = self.cmb_scenario.selection() else { return };
        if i < Preset::all().len() {
            let preset = &Preset::all()[i];
            self.suppress_custom.set(true);
            {
                let mut cfg = state.cfg.borrow_mut();
                self.save(&mut cfg);
                preset.apply(&mut cfg);
                self.load_fields(&cfg);
            }
            self.suppress_custom.set(false);
        }
        self.refresh_preview(state);
    }

    /// Live effective-command preview. Uses the control-pure `write_fields`
    /// on a throwaway clone -- NOT `save()`, which clears the webhook field
    /// and would drop the typed URL into the discarded clone.
    pub fn refresh_preview(&self, state: &super::WizardState) {
        let mut cfg = state.cfg.borrow().clone();
        self.write_fields(&mut cfg);
        self.txt_effective.set_text(&crate::procdump::build_args(&cfg));
    }

    /// Shows the bitness the MONITOR will resolve, using the same code path,
    /// so the preview cannot disagree with runtime behaviour.
    fn update_bitness(&self, cfg: &Config, procdump_path: &str) {
        self.lbl_bitness.set_text(&bitness_text(cfg, procdump_path));
    }

    pub fn browse_procdump_path(&self, parent: nwg::ControlHandle) {
        let mut dialog = nwg::FileDialog::default();
        let built = nwg::FileDialog::builder()
            .title("Select ProcDump executable")
            .action(nwg::FileDialogAction::Open)
            .filters("Exe(*.exe)")
            .build(&mut dialog)
            .is_ok();
        if built && dialog.run(Some(parent)) {
            if let Ok(p) = dialog.get_selected_item() {
                self.txt_procdump_path.set_text(&p.to_string_lossy());
            }
        }
    }

    pub fn browse_dump_dir(&self, parent: nwg::ControlHandle) {
        let mut dialog = nwg::FileDialog::default();
        let built = nwg::FileDialog::builder()
            .title("Select dump directory")
            .action(nwg::FileDialogAction::OpenDirectory)
            .build(&mut dialog)
            .is_ok();
        if built && dialog.run(Some(parent)) {
            if let Ok(p) = dialog.get_selected_item() {
                self.txt_dump_dir.set_text(&p.to_string_lossy());
            }
        }
    }

    // ---- load / save --------------------------------------------------------

    /// Field population shared by load() and preset application.
    fn load_fields(&self, cfg: &Config) {
        let prev = self.suppress_custom.replace(true);

        let idx = Preset::all().iter().position(|p| p.name == cfg.scenario);
        self.cmb_scenario.set_selection(Some(idx.unwrap_or(Preset::all().len())));

        let dt_idx = DUMP_TYPES.iter().position(|s| *s == cfg.dump_type).unwrap_or(0);
        self.cmb_dump_type.set_selection(Some(dt_idx));

        set_checked(&self.chk_exception, cfg.dump_on_exception);
        set_checked(&self.chk_terminate, cfg.dump_on_terminate);
        set_checked(&self.chk_hang, cfg.hang_window_seconds > 0);

        self.txt_cpu.set_text(&cfg.cpu_threshold.to_string());
        self.txt_cpu_low.set_text(&cfg.cpu_low_threshold.to_string());
        self.txt_cpu_dur.set_text(&cfg.cpu_duration_seconds.to_string());
        self.txt_count.set_text(&cfg.max_dumps.to_string());
        self.txt_mem.set_text(&cfg.memory_commit_mb.to_string());
        self.txt_procdump_path.set_text(&cfg.proc_dump_path);
        self.txt_dump_dir.set_text(&cfg.dump_directory);

        self.suppress_custom.set(prev);
    }

    pub fn load(&self, cfg: &Config) {
        let prev = self.suppress_custom.replace(true);

        // Target: select the entry matching the saved name, else stash it as
        // a manual override so save() round-trips it unchanged.
        let mut found = false;
        if !cfg.target_name.is_empty() {
            let want_service = cfg.target_type == TargetType::Service;
            if let Some(i) = self
                .entries
                .borrow()
                .iter()
                .position(|e| e.is_service == want_service && e.name.eq_ignore_ascii_case(&cfg.target_name))
            {
                self.cmb_target.set_selection(Some(i));
                found = true;
            }
        }
        *self.manual_target.borrow_mut() =
            if found { String::new() } else { cfg.target_name.clone() };

        self.load_fields(cfg);

        self.txt_task_name.set_text(&cfg.task_name);
        set_checked(&self.chk_email, cfg.email_enabled);
        self.txt_to.set_text(&cfg.to_address);
        self.txt_to.set_enabled(cfg.email_enabled);
        set_checked(&self.chk_webhook, cfg.webhook_enabled);
        self.txt_webhook.set_text("");
        self.txt_webhook.set_placeholder_text(
            if cfg.encrypted_webhook_url_blob.is_empty() { None } else { Some("(unchanged)") },
        );
        self.txt_webhook.set_enabled(cfg.webhook_enabled);
        set_checked(&self.chk_autocollect, cfg.auto_collect_on_dump);

        self.update_bitness(cfg, &cfg.proc_dump_path);
        self.suppress_custom.set(prev);
    }

    /// Writes every on-screen field into `cfg`. CONTROL-PURE: never mutates a
    /// control and never encrypts/clears secrets, so it is safe to call on a
    /// throwaway clone (refresh_preview) as often as needed. `save()` layers
    /// the side effects on top for the real-persist paths.
    fn write_fields(&self, cfg: &mut Config) {
        let (name, ttype) = self.effective_target();
        // Auto task name follows the target while the user hasn't customized
        // the name (same rule the old Target page applied). Computed against
        // the PREVIOUS target_name still in cfg.
        let typed_task = task::sanitize_task_name(&self.txt_task_name.text());
        if typed_task == task::auto_task_name(&cfg.target_name)
            || typed_task == "ProcDump Monitor"
            || typed_task.is_empty()
        {
            cfg.task_name = task::auto_task_name(&name);
        } else {
            cfg.task_name = typed_task;
        }
        // Assigns name/type and drops a cached target_path belonging to the
        // PREVIOUS target. Must live here, not in save(): the clear needs the
        // old name/type, which this line is about to overwrite. Cheap and
        // control-pure — the matching capture (which hits the registry) is in
        // save() instead, because this runs on every keystroke.
        bitness::set_target(cfg, &name, ttype);

        cfg.scenario = match self.cmb_scenario.selection_string() {
            Some(s) if s != "Custom" => s,
            _ => String::new(),
        };
        cfg.dump_type = self.cmb_dump_type.selection_string().unwrap_or_else(|| "Full".into());
        cfg.dump_on_exception = checked(&self.chk_exception);
        cfg.dump_on_terminate = checked(&self.chk_terminate);
        cfg.hang_window_seconds = if checked(&self.chk_hang) { 1 } else { 0 };
        cfg.cpu_threshold = parse_i32(&self.txt_cpu);
        cfg.cpu_low_threshold = parse_i32(&self.txt_cpu_low);
        cfg.cpu_duration_seconds = parse_i32(&self.txt_cpu_dur);
        cfg.max_dumps = parse_i32(&self.txt_count).max(1);
        cfg.memory_commit_mb = parse_i32(&self.txt_mem);
        cfg.proc_dump_path = self.txt_procdump_path.text().trim().to_string();
        cfg.dump_directory = self.txt_dump_dir.text().trim().to_string();

        cfg.email_enabled = checked(&self.chk_email);
        cfg.to_address = self.txt_to.text().trim().to_string();
        cfg.webhook_enabled = checked(&self.chk_webhook);
        cfg.auto_collect_on_dump = checked(&self.chk_autocollect);
    }

    /// Never blocks a page switch; email validation runs in create_task().
    /// This is the real-persist path: it applies the control side effects
    /// write_fields deliberately omits (sync the auto task-name box, DPAPI-
    /// protect a freshly typed webhook URL and then clear the field).
    pub fn save(&self, cfg: &mut Config) -> bool {
        self.write_fields(cfg);
        // Capture the image path so bitness survives the target not running.
        // Here and NOT in write_fields: this shells to reg.exe for a Service
        // target, and write_fields runs on every preview refresh (i.e. every
        // keystroke in the option text boxes). write_fields has already
        // cleared any path belonging to a previous target, so this cannot
        // re-bless a stale one.
        //
        // ponytail: THIS ONE LINE is the entire production writer of
        // cfg.target_path. Comment it out and all 107 unit tests still pass
        // (demonstrated in review) while the feature dies silently — bitness
        // degrades to the pre-plan runtime-only behaviour with no error. No
        // unit test can cover it: reaching save() needs live nwg controls.
        // Ceiling accepted deliberately. Upgrade path: assert TargetPath in
        // scripts/gui-e2e.ps1 after a Save Config click (Task 8 owns that);
        // until then this line is guarded by review only.
        bitness::capture_target_path(cfg);
        self.txt_task_name.set_text(&cfg.task_name);

        let url = self.txt_webhook.text();
        if !url.is_empty() {
            cfg.encrypted_webhook_url_blob =
                crate::secrets::protect(&url, crate::secrets::WEBHOOK_ENTROPY);
            self.txt_webhook.set_text("");
            self.txt_webhook.set_placeholder_text(Some("(unchanged)"));
        }
        true
    }

    pub fn on_email_toggled(&self) {
        self.txt_to.set_enabled(checked(&self.chk_email));
    }

    pub fn on_webhook_toggled(&self) {
        self.txt_webhook.set_enabled(checked(&self.chk_webhook));
    }

    /// Port of the old Notify save() gate, run only when installing: From
    /// (in cfg, set via SMTP dialog) + at least one valid To.
    fn validate_notify(&self, cfg: &Config) -> bool {
        if !cfg.email_enabled {
            return true;
        }
        let to_list = crate::notify::split_addresses(&cfg.to_address);
        let cc_list = crate::notify::split_addresses(&cfg.cc_address);
        let bad = to_list.iter().chain(cc_list.iter()).any(|a| !a.contains('@'));
        if cfg.from_address.is_empty() || to_list.is_empty() || bad {
            nwg::modal_error_message(
                self.chk_email.handle,
                "Invalid email settings",
                "Email notifications are enabled but incomplete: set a From address (SMTP... dialog) and at least one To address containing '@'.",
            );
            return false;
        }
        true
    }

    // ---- Footer actions (called from the shell) ------------------------------

    fn run_own_verb(verb: &str) -> (bool, String) {
        let out = Command::new(paths::exe_path())
            .args([verb, "--config", &paths::config_path().display().to_string()])
            .output();
        match out {
            Ok(o) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                (o.status.success(), text.trim().to_string())
            }
            Err(e) => (false, format!("cannot run {verb}: {e}")),
        }
    }

    fn record_action(&self, state: &super::WizardState, ok: bool, msg: &str) {
        *self.action_error.borrow_mut() =
            if ok { None } else { Some(msg.to_string()) };
        if ok {
            crate::logger::log("GUI", msg);
        }
        self.refresh_status(state);
    }

    /// Create/Update: save all fields -> validate -> persist -> install.
    pub fn create_task(&self, state: &super::WizardState) {
        {
            let mut cfg = state.cfg.borrow_mut();
            self.save(&mut cfg);
        }
        if !self.validate_notify(&state.cfg.borrow()) {
            return;
        }
        let path = paths::config_path();
        if let Err(e) = state.cfg.borrow_mut().save(&path) {
            self.record_action(state, false, &format!("Failed to save config: {e}"));
            return;
        }
        let (ok, msg) = Self::run_own_verb("install");
        self.record_action(state, ok, &msg);
    }

    pub fn run_task(&self, state: &super::WizardState) {
        let (ok, msg) = Self::run_own_verb("start");
        self.record_action(state, ok, &msg);
    }

    pub fn stop_task(&self, state: &super::WizardState) {
        let (ok, msg) = Self::run_own_verb("stop");
        self.record_action(state, ok, &msg);
    }

    pub fn remove_task(&self, state: &super::WizardState) {
        let (ok, msg) = Self::run_own_verb("uninstall");
        self.record_action(state, ok, &msg);
    }

    pub fn save_config_only(&self, state: &super::WizardState) {
        {
            let mut cfg = state.cfg.borrow_mut();
            self.save(&mut cfg);
        }
        let path = paths::config_path();
        let res = state.cfg.borrow_mut().save(&path);
        match res {
            Ok(()) => self.record_action(state, true, "Config saved."),
            Err(e) => self.record_action(state, false, &format!("Failed to save config: {e}")),
        }
    }

    pub fn open_dump_folder(&self, state: &super::WizardState) {
        let dir = state.cfg.borrow().dump_directory.clone();
        let _ = Command::new("explorer.exe").arg(&dir).spawn();
    }

    pub fn view_logs(&self) {
        let _ = Command::new("notepad.exe").arg(paths::log_path()).spawn();
    }

    pub fn copy_args(&self, state: &super::WizardState) {
        let args = crate::procdump::build_args(&state.cfg.borrow());
        nwg::Clipboard::set_data_text(&self.txt_effective, &args);
    }

    pub fn open_task_scheduler(&self) {
        let _ = Command::new("mmc.exe").arg("taskschd.msc").spawn();
    }

    // ---- Live status ---------------------------------------------------------

    /// Refresh the four status rows from schtasks + health.json. Called on
    /// page entry, after every footer action, and by the shell's poll timer
    /// while this page is visible.
    pub fn refresh_status(&self, state: &super::WizardState) {
        let (task_name, dump_dir) = {
            let cfg = state.cfg.borrow();
            (task::sanitize_task_name(&cfg.task_name), cfg.dump_directory.clone())
        };

        // Row 1: scheduled task, verified via schtasks.
        if !task_name.is_empty() && task::exists(&task_name) {
            let st = task::query_status(&task_name);
            self.lbl_st_task
                .set_text(&format!("\u{2713} Scheduled task \"{task_name}\" \u{2014} {}", st.state));
            theme::set_status_color(&self.lbl_st_task.handle, theme::GOOD);
        } else {
            self.lbl_st_task
                .set_text("\u{25CB} No scheduled task installed yet \u{2014} click Create Task");
            theme::set_status_color(&self.lbl_st_task.handle, theme::MUTED);
        }

        // Rows 2-4: monitor heartbeat from health.json.
        let hp = paths::health_path();
        if hp.exists() {
            let h = health::load(&hp);
            let age = chrono::DateTime::parse_from_rfc3339(&h.last_cycle_utc)
                .ok()
                .map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds());
            match age {
                Some(a) if (0..=60).contains(&a) => {
                    let attach = if h.proc_dump_pid != 0 {
                        format!("ProcDump attached (PID {})", h.proc_dump_pid)
                    } else {
                        "ProcDump not attached yet".to_string()
                    };
                    self.lbl_st_monitor.set_text(&format!(
                        "\u{25CF} Monitor running (PID {}) \u{2014} heartbeat {a}s ago \u{2014} {attach}",
                        h.monitor_pid
                    ));
                    theme::set_status_color(&self.lbl_st_monitor.handle, theme::GOOD);
                }
                Some(_) | None => {
                    self.lbl_st_monitor.set_text(&format!(
                        "\u{25CB} Monitor not running \u{2014} last heartbeat {}",
                        if h.last_cycle_utc.is_empty() { "never" } else { &h.last_cycle_utc }
                    ));
                    theme::set_status_color(&self.lbl_st_monitor.handle, theme::MUTED);
                }
            }
            let disk = if h.free_disk_mb > 0 {
                format!(" \u{2014} {:.1} GB free", h.free_disk_mb as f64 / 1024.0)
            } else {
                String::new()
            };
            let latest = if h.last_dump_file_name.is_empty() {
                String::new()
            } else {
                format!(" \u{2014} latest: {}", h.last_dump_file_name)
            };
            self.lbl_st_dumps
                .set_text(&format!("{} dump(s) captured{latest}{disk}", h.total_dump_count));
            theme::set_status_color(
                &self.lbl_st_dumps.handle,
                if h.disk_space_low { theme::WARN } else { theme::MUTED },
            );

            if let Some(err) = self.action_error.borrow().as_deref() {
                self.lbl_st_alert.set_text(&format!("\u{2715} {err}"));
                theme::set_status_color(&self.lbl_st_alert.handle, theme::BAD);
            } else if !h.last_error.is_empty() {
                self.lbl_st_alert.set_text(&format!("\u{26A0} Monitor: {}", h.last_error));
                theme::set_status_color(&self.lbl_st_alert.handle, theme::WARN);
            } else {
                self.lbl_st_alert.set_text("");
            }
        } else {
            self.lbl_st_monitor
                .set_text("\u{25CB} Monitor has not run yet (no health.json)");
            theme::set_status_color(&self.lbl_st_monitor.handle, theme::MUTED);
            let dumps = std::fs::read_dir(&dump_dir)
                .map(|rd| {
                    rd.flatten()
                        .filter(|e| {
                            e.path()
                                .extension()
                                .map(|x| x.eq_ignore_ascii_case("dmp"))
                                .unwrap_or(false)
                        })
                        .count()
                })
                .unwrap_or(0);
            self.lbl_st_dumps.set_text(&format!("{dumps} dump(s) in the dump folder"));
            theme::set_status_color(&self.lbl_st_dumps.handle, theme::MUTED);
            match self.action_error.borrow().as_deref() {
                Some(err) => {
                    self.lbl_st_alert.set_text(&format!("\u{2715} {err}"));
                    theme::set_status_color(&self.lbl_st_alert.handle, theme::BAD);
                }
                None => self.lbl_st_alert.set_text(""),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway ProcDump directory. Same counter trick as bitness.rs's copy:
    /// cargo runs tests in parallel and several of these share a file list.
    fn dir_with(files: &[&str]) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("pdm_lbl_{n}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for f in files {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        d
    }

    /// Path handed to `bitness_text` — it takes the ProcDump EXE path and uses
    /// the parent, which is what the txt_procdump_path box holds.
    fn pd_exe(d: &std::path::Path) -> String {
        d.join("procdump64.exe").to_string_lossy().to_string()
    }

    fn my_process_name() -> String {
        std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().to_string()
    }

    // ---- bitness_label: pure, exact strings -------------------------------
    //
    // Every assertion below is on the FULL rendered string, not `contains`.
    // A contains-only test cannot fail here: drop the Unknown arm entirely and
    // the fall-through still emits a string containing "Unknown".

    #[test]
    fn label_names_the_source_when_resolved() {
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        let c = bitness::select_binary(bitness::Bitness::X86, &d, true);
        assert_eq!(
            bitness_label(bitness::Bitness::X86, "PE header", &c),
            "32-bit process -> procdump.exe (via PE header)"
        );
    }

    #[test]
    fn label_keeps_the_word_unknown_when_unresolved() {
        // The requirement verbatim, in BOTH directory shapes select_binary can
        // hit for Unknown. A svchost-hosted 32-bit service cannot be resolved
        // from a PE, and the label must not read as a verified 64-bit answer.
        let both = dir_with(&["procdump.exe", "procdump64.exe"]);
        let c = bitness::select_binary(bitness::Bitness::Unknown, &both, true);
        let t = bitness_label(bitness::Bitness::Unknown, "unresolved", &c);
        assert_eq!(
            t,
            "Unknown bitness -> procdump64.exe (default) \
             - could not determine target bitness; verify manually."
        );
        assert!(t.contains("Unknown"), "the word Unknown must survive: {t}");

        // Only the 32-bit binary present: Unknown AND a warning. The warning
        // arm wins, so this is the second place "Unknown" has to survive.
        let only32 = dir_with(&["procdump.exe"]);
        let c = bitness::select_binary(bitness::Bitness::Unknown, &only32, true);
        let t = bitness_label(bitness::Bitness::Unknown, "unresolved", &c);
        assert_eq!(
            t,
            "Unknown bitness -> procdump.exe \
             - procdump64.exe not found; using procdump.exe as fallback."
        );
        assert!(t.contains("Unknown"), "the word Unknown must survive: {t}");
    }

    #[test]
    fn label_shows_a_select_binary_warning() {
        let d = dir_with(&["procdump64.exe"]);
        let c = bitness::select_binary(bitness::Bitness::X86, &d, true);
        assert_eq!(
            bitness_label(bitness::Bitness::X86, "PE header", &c),
            "32-bit process -> procdump64.exe (fallback) \
             - procdump.exe not found - falling back to procdump64.exe."
        );
    }

    #[test]
    fn label_reports_a_32bit_os() {
        // The half of the old bug that hardcoded `os_is_64: true`: on a 32-bit
        // OS the label claimed procdump64.exe would be used. This host is x64,
        // so os_is_64 is injected here rather than mutating the environment --
        // which is exactly why bitness_label is split out of bitness_text.
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        let c = bitness::select_binary(bitness::Bitness::X64, &d, false);
        assert_eq!(
            bitness_label(bitness::Bitness::X64, "PE header", &c),
            "32-bit OS -> procdump.exe (via PE header)"
        );
    }

    // ---- bitness_text: the wiring update_bitness uses ---------------------

    #[test]
    fn text_is_empty_for_an_empty_target() {
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        assert_eq!(bitness_text(&Config::default(), &pd_exe(&d)), "");

        // Service type too: this is the guard that keeps an empty name from
        // spawning reg.exe against the bare ...\Services key. The empty return
        // is the observable half; the absent spawn is inspection-only.
        let mut c = Config::default();
        c.target_type = TargetType::Service;
        c.target_name = "   ".into();
        assert_eq!(bitness_text(&c, &pd_exe(&d)), "");
    }

    #[test]
    fn text_resolves_a_running_process_from_its_pe() {
        // Also kills a `bitness::resolve` -> `bitness::detect` regression: the
        // runtime path would render "(via running process)".
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = my_process_name();
        assert_eq!(
            bitness_text(&c, &pd_exe(&d)),
            "64-bit process -> procdump64.exe (via PE header)"
        );
    }

    #[test]
    fn text_resolves_a_service_target() {
        // THE defect this task fixes: `detect` takes a process name, so it
        // returned Unknown for every service. Spooler is a standalone exe
        // (not svchost-hosted), so the PE path must answer.
        if bitness::service_image_path("Spooler").is_none() {
            eprintln!("skipping: Spooler not present on this host");
            return;
        }
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        let mut c = Config::default();
        c.target_type = TargetType::Service;
        c.target_name = "Spooler".into();
        assert_eq!(
            bitness_text(&c, &pd_exe(&d)),
            "64-bit process -> procdump64.exe (via PE header)"
        );
    }

    #[test]
    fn text_is_unresolved_for_a_target_that_cannot_be_found() {
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = "PdmDefinitelyNotRunning.exe".into();
        assert_eq!(
            bitness_text(&c, &pd_exe(&d)),
            "Unknown bitness -> procdump64.exe (default) \
             - could not determine target bitness; verify manually."
        );
    }
}

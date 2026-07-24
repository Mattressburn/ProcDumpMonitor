use crate::config::Config;
use crate::procdump::Preset;
use native_windows_gui as nwg;
use std::cell::Cell;
use std::rc::Rc;

use super::theme;

const DUMP_TYPES: [&str; 4] = ["Full", "MiniPlus", "Mini", "ThreadDump"];

pub struct ProcDumpPage {
    // Kept alive only so the section-header font isn't freed out from under
    // the header Labels still displaying it (Font's Drop frees the HFONT;
    // see page_task.rs's `bold_font` for the same pattern).
    #[allow(dead_code)]
    header_font: nwg::Font,
    /// Pure caption labels -- section headers and field captions -- built
    /// for their on-screen text only, never read back. Held here so they
    /// outlive `build()` (Label's Drop destroys its window; a local that's
    /// dropped at the end of `build()` silently vanishes from the screen --
    /// this is the bug the CONTROL LIFETIME rule guards against).
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    pub cmb_scenario: nwg::ComboBox<String>,
    pub txt_effective: nwg::TextInput,
    pub lbl_bitness: nwg::Label,
    pub txt_procdump_path: nwg::TextInput,
    pub btn_browse_pd: nwg::Button,
    pub txt_dump_dir: nwg::TextInput,
    pub btn_browse_dir: nwg::Button,
    pub cmb_dump_type: nwg::ComboBox<String>,
    pub chk_exception: nwg::CheckBox,
    pub chk_hang: nwg::CheckBox,
    pub chk_terminate: nwg::CheckBox,
    pub txt_cpu: nwg::TextInput,
    pub txt_cpu_low: nwg::TextInput,
    pub txt_cpu_dur: nwg::TextInput,
    pub txt_count: nwg::TextInput,
    pub chk_cpu_per_unit: nwg::CheckBox,
    pub txt_mem: nwg::TextInput,
    pub chk_clone: nwg::CheckBox,
    pub chk_avoid: nwg::CheckBox,
    pub chk_overwrite: nwg::CheckBox,
    pub chk_wait: nwg::CheckBox,
    pub txt_restart_delay: nwg::TextInput,
    pub txt_min_disk: nwg::TextInput,
    pub txt_perf_counter: nwg::TextInput,
    pub txt_perf_threshold: nwg::TextInput,
    pub txt_filter_include: nwg::TextInput,
    pub txt_filter_exclude: nwg::TextInput,
    pub chk_wer: nwg::CheckBox,
    pub txt_avoid_terminate: nwg::TextInput,

    /// Every control whose change should flip the scenario combo to
    /// "Custom" -- everything except cmb_scenario itself (has its own
    /// handler) and the two read-only/output controls (txt_effective,
    /// lbl_bitness).
    option_handles: Vec<nwg::ControlHandle>,

    /// True while `load()` is repopulating controls from a `Config` (either
    /// the page's own load or the load half of a preset apply) so the
    /// change events those writes trigger aren't misread as user edits.
    /// See `load()`'s save/restore pattern -- it nests correctly with the
    /// explicit set/unset pair in `on_scenario_selected`.
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

/// Section-header caption: full-width, Segoe UI Semibold 15px (per the
/// binding design system). `font` must outlive the returned Label.
fn mk_header<P: Into<nwg::ControlHandle> + Copy>(parent: P, text: &str, y: i32, font: &nwg::Font) -> nwg::Label {
    let mut l = nwg::Label::default();
    nwg::Label::builder()
        .text(text)
        .position((32, y))
        .size((616, 22))
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

fn mk_combo<P: Into<nwg::ControlHandle> + Copy>(
    parent: P,
    pos: (i32, i32),
    size: (i32, i32),
) -> nwg::ComboBox<String> {
    let mut c = nwg::ComboBox::default();
    nwg::ComboBox::builder().position(pos).size(size).parent(parent).build(&mut c).unwrap();
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

fn parse_i64(t: &nwg::TextInput) -> i64 {
    t.text().trim().parse::<i64>().unwrap_or(0)
}

/// This is the dense page: 29 controls in a single 680x456 frame. Every row
/// below was hand-budgeted against that height with a running `y` cursor --
/// see docs/plans + .superpowers/sdd/task-3-report.md for the row-by-row
/// math. Field widths use ~7px/char for caption labels (the original file's
/// own worst-case label -- "CPU% / Low% / Dur(s) / Max:" at 175px wide --
/// shipped without clipping at that ratio, so it's an empirically safe
/// floor, not the spec's more conservative ~9px/char ceiling). TextInputs
/// are sized tighter than that -- unlike static labels they scroll instead
/// of clipping, so density there costs nothing.
pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> ProcDumpPage {
    let header_font = theme::semibold(15);
    let mut captions: Vec<nwg::Label> = Vec::new();

    const PAD: i32 = 32;
    const FULL_W: i32 = 616; // 680 frame width - 2*32 padding
    const ROW_H: i32 = 34; // dense-page minimum row pitch
    const FIELD_H: i32 = 26;
    // Shared wizard field column: PAD(32) + label col(190) + gap(10). Every
    // row's FIRST control starts here so ProcDump's field column lines up with
    // the single column all the other pages use (frame-rel x=232). Labels live
    // in the label column (x=32, w<=190). Packed rows flow extra controls
    // rightward from 232 and must stay inside the right margin (x <= 648);
    // TextInputs scroll rather than clip, so they're sized tight to make room.
    const FIELD_X: i32 = 232;

    let mut y = PAD;

    // ---- Section: Scenario --------------------------------------------
    captions.push(mk_header(parent, "Scenario", y, &header_font));
    y += 22 + 8;

    captions.push(mk_label(parent, "Scenario:", (PAD, y - 2), (190, 20)));
    let cmb_scenario = mk_combo(parent, (FIELD_X, y), (300, 26));
    {
        let mut names: Vec<String> = Preset::all().iter().map(|p| p.name.to_string()).collect();
        names.push("Custom".into());
        cmb_scenario.set_collection(names);
    }
    // Bitness readout: a status/warning readout whose real text (set in
    // `load()`) can run to 100+ chars (summary + a fallback warning -- see
    // bitness.rs), so it gets the full content width on its own hint line
    // right under the scenario row instead of being squeezed beside the
    // combo. It's still a static Label (can't scroll like a TextInput), so
    // width is the only lever available; full width is the most this frame
    // can offer it.
    let lbl_bitness = mk_label(parent, "", (PAD, y + 27), (FULL_W, 18));
    theme::register_muted(&lbl_bitness.handle);
    y += 46; // combo row (26) + tight gap + hint line (18) + tight gap

    // Effective command preview -- kept a single-line TextInput (the public
    // field's type is frozen); full width per the layout brief.
    let txt_effective = mk_text(parent, (PAD, y), (FULL_W, FIELD_H), true);
    y += ROW_H;

    // ---- Section: Configuration (output + triggers) --------------------
    y += 16;
    captions.push(mk_header(parent, "Configuration", y, &header_font));
    y += 22 + 8;

    captions.push(mk_label(parent, "ProcDump path:", (PAD, y - 2), (190, 20)));
    let txt_procdump_path = mk_text(parent, (FIELD_X, y), (290, FIELD_H), false);
    let btn_browse_pd = mk_button(parent, "Browse...", (FIELD_X + 298, y - 2), (110, 30));
    y += ROW_H;

    captions.push(mk_label(parent, "Dump directory:", (PAD, y - 2), (190, 20)));
    let txt_dump_dir = mk_text(parent, (FIELD_X, y), (290, FIELD_H), false);
    let btn_browse_dir = mk_button(parent, "Browse...", (FIELD_X + 298, y - 2), (110, 30));
    y += ROW_H;

    // Dump type combo + the two dump-*trigger* checkboxes pack from x=232.
    // The third trigger, -t, doesn't fit here once the row starts at the
    // shared column, so it reflows onto the CPU row below (which has slack
    // after its four tiny numeric fields).
    captions.push(mk_label(parent, "Dump type:", (PAD, y - 2), (190, 20)));
    let cmb_dump_type = mk_combo(parent, (FIELD_X, y), (95, 26));
    cmb_dump_type.set_collection(DUMP_TYPES.iter().map(|s| s.to_string()).collect());
    let chk_exception = mk_check(parent, "-e unhandled exception", (FIELD_X + 103, y), (180, 22));
    let chk_hang = mk_check(parent, "-h hung window", (FIELD_X + 291, y), (118, 22));
    y += ROW_H;

    captions.push(mk_label(parent, "CPU% / Low% / Dur / Max:", (PAD, y - 2), (190, 20)));
    let txt_cpu = mk_text(parent, (FIELD_X, y), (40, FIELD_H), false);
    let txt_cpu_low = mk_text(parent, (FIELD_X + 46, y), (40, FIELD_H), false);
    let txt_cpu_dur = mk_text(parent, (FIELD_X + 92, y), (40, FIELD_H), false);
    let txt_count = mk_text(parent, (FIELD_X + 138, y), (40, FIELD_H), false);
    // Reflowed here from the Dump type row (see above).
    let chk_terminate = mk_check(parent, "-t on terminate", (FIELD_X + 184, y), (118, 22));
    captions.push(mk_label(parent, "Incl (-f):", (FIELD_X + 306, y - 2), (52, 20)));
    let txt_filter_include = mk_text(parent, (FIELD_X + 360, y), (54, FIELD_H), false);
    y += ROW_H;

    captions.push(mk_label(parent, "Commit MB (-m):", (PAD, y - 2), (190, 20)));
    let txt_mem = mk_text(parent, (FIELD_X, y), (66, FIELD_H), false);
    let chk_clone = mk_check(parent, "-r clone", (FIELD_X + 72, y), (78, 22));
    let chk_avoid = mk_check(parent, "-a avoid outage", (FIELD_X + 156, y), (126, 22));
    let chk_overwrite = mk_check(parent, "-o overwrite", (FIELD_X + 288, y), (110, 22));
    y += ROW_H;

    // Launch/integration checkboxes, packed from the shared field column
    // (F2b: this row used to start at the label column).
    let chk_wait = mk_check(parent, "-w wait for launch", (FIELD_X, y), (140, 22));
    let chk_wer = mk_check(parent, "-wer WER integration", (FIELD_X + 146, y), (162, 22));
    let chk_cpu_per_unit = mk_check(parent, "-u per-CPU", (FIELD_X + 314, y), (96, 22));
    y += ROW_H;

    captions.push(mk_label(parent, "Restart delay (s):", (PAD, y - 2), (190, 20)));
    let txt_restart_delay = mk_text(parent, (FIELD_X, y), (64, FIELD_H), false);
    captions.push(mk_label(parent, "Min free disk MB:", (FIELD_X + 70, y - 2), (140, 20)));
    let txt_min_disk = mk_text(parent, (FIELD_X + 214, y), (66, FIELD_H), false);
    captions.push(mk_label(parent, "Excl (-fx):", (FIELD_X + 286, y - 2), (66, 20)));
    let txt_filter_exclude = mk_text(parent, (FIELD_X + 356, y), (56, FIELD_H), false);
    y += ROW_H;

    // Perf counter/threshold pair, plus the reflowed Avoid-term pair (moved
    // off the launch-checkbox row, which no longer has room once it starts at
    // the shared field column).
    captions.push(mk_label(parent, "Perf counter (-p):", (PAD, y - 2), (190, 20)));
    let txt_perf_counter = mk_text(parent, (FIELD_X, y), (88, FIELD_H), false);
    captions.push(mk_label(parent, "Threshold (-pl):", (FIELD_X + 96, y - 2), (116, 20)));
    let txt_perf_threshold = mk_text(parent, (FIELD_X + 216, y), (70, FIELD_H), false);
    captions.push(mk_label(parent, "Avoid (s):", (FIELD_X + 292, y - 2), (72, 20)));
    let txt_avoid_terminate = mk_text(parent, (FIELD_X + 368, y), (44, FIELD_H), false);
    y += ROW_H;
    // Final row bottom sits at ~452 logical px, inside the 456-tall frame;
    // the rightmost control (avoid field) ends at x=644, inside the 648 margin.
    let _ = y;

    let option_handles = vec![
        txt_procdump_path.handle,
        txt_dump_dir.handle,
        cmb_dump_type.handle,
        chk_exception.handle,
        chk_hang.handle,
        chk_terminate.handle,
        txt_cpu.handle,
        txt_cpu_low.handle,
        txt_cpu_dur.handle,
        txt_count.handle,
        chk_cpu_per_unit.handle,
        txt_mem.handle,
        chk_clone.handle,
        chk_avoid.handle,
        chk_overwrite.handle,
        chk_wait.handle,
        txt_restart_delay.handle,
        txt_min_disk.handle,
        txt_perf_counter.handle,
        txt_perf_threshold.handle,
        txt_filter_include.handle,
        txt_filter_exclude.handle,
        chk_wer.handle,
        txt_avoid_terminate.handle,
    ];

    ProcDumpPage {
        header_font,
        captions,
        cmb_scenario,
        txt_effective,
        lbl_bitness,
        txt_procdump_path,
        btn_browse_pd,
        txt_dump_dir,
        btn_browse_dir,
        cmb_dump_type,
        chk_exception,
        chk_hang,
        chk_terminate,
        txt_cpu,
        txt_cpu_low,
        txt_cpu_dur,
        txt_count,
        chk_cpu_per_unit,
        txt_mem,
        chk_clone,
        chk_avoid,
        chk_overwrite,
        chk_wait,
        txt_restart_delay,
        txt_min_disk,
        txt_perf_counter,
        txt_perf_threshold,
        txt_filter_include,
        txt_filter_exclude,
        chk_wer,
        txt_avoid_terminate,
        option_handles,
        suppress_custom: Cell::new(false),
    }
}

impl ProcDumpPage {
    /// Wired into gui::run's event handler for every control in
    /// `option_handles` (OnTextInput / OnButtonClick / OnComboxBoxSelection).
    pub fn is_option_control(&self, h: nwg::ControlHandle) -> bool {
        self.option_handles.contains(&h)
    }

    /// Any manual option edit flips the scenario to Custom. No-ops while a
    /// `load()` (page load or preset apply) is in progress -- those control
    /// writes aren't user edits.
    pub fn on_option_changed(&self, state: &super::WizardState) {
        if self.suppress_custom.get() {
            return;
        }
        self.cmb_scenario.set_selection(Some(Preset::all().len()));
        state.cfg.borrow_mut().scenario = String::new();
        self.refresh_preview(state);
    }

    /// Wired to cmb_scenario's OnComboxBoxSelection.
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
                self.save(&mut cfg); // capture path fields the user already set
                preset.apply(&mut cfg);
                self.load(&cfg); // push preset values back into controls
            }
            self.suppress_custom.set(false);
        }
        self.refresh_preview(state);
    }

    /// Live effective-command preview. Called from every option handler and
    /// once explicitly by callers of `load()` after it returns.
    pub fn refresh_preview(&self, state: &super::WizardState) {
        let mut cfg = state.cfg.borrow().clone();
        self.save(&mut cfg);
        self.txt_effective.set_text(&crate::procdump::build_args(&cfg));
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

    /// Self-guarding: every control write in here can trigger a change
    /// event (OnTextInput fires on programmatic WM_SETTEXT for single-line
    /// edits, which is what TextInput is). Wrapping the whole body means
    /// ANY caller of `load()` -- not just `on_scenario_selected`, which
    /// wraps its own call too -- is protected against that cascade
    /// immediately flipping the combo back to "Custom". Save/restore (not
    /// set-false) so nesting inside `on_scenario_selected`'s own guard
    /// still leaves it `true` on return, matching that caller's contract.
    pub fn load(&self, cfg: &Config) {
        let prev = self.suppress_custom.replace(true);

        let idx = Preset::all().iter().position(|p| p.name == cfg.scenario);
        self.cmb_scenario.set_selection(Some(idx.unwrap_or(Preset::all().len())));

        self.txt_procdump_path.set_text(&cfg.proc_dump_path);
        self.txt_dump_dir.set_text(&cfg.dump_directory);

        let dt_idx = DUMP_TYPES.iter().position(|s| *s == cfg.dump_type).unwrap_or(0);
        self.cmb_dump_type.set_selection(Some(dt_idx));

        set_checked(&self.chk_exception, cfg.dump_on_exception);
        set_checked(&self.chk_terminate, cfg.dump_on_terminate);
        set_checked(&self.chk_hang, cfg.hang_window_seconds > 0);

        self.txt_cpu.set_text(&cfg.cpu_threshold.to_string());
        self.txt_cpu_low.set_text(&cfg.cpu_low_threshold.to_string());
        self.txt_cpu_dur.set_text(&cfg.cpu_duration_seconds.to_string());
        self.txt_count.set_text(&cfg.max_dumps.to_string());
        set_checked(&self.chk_cpu_per_unit, cfg.cpu_per_unit);
        self.txt_mem.set_text(&cfg.memory_commit_mb.to_string());

        set_checked(&self.chk_clone, cfg.use_clone);
        set_checked(&self.chk_avoid, cfg.avoid_outage);
        set_checked(&self.chk_overwrite, cfg.overwrite_existing);
        set_checked(&self.chk_wait, cfg.wait_for_process);

        self.txt_restart_delay.set_text(&cfg.restart_delay_seconds.to_string());
        self.txt_min_disk.set_text(&cfg.min_free_disk_mb.to_string());

        self.txt_perf_counter.set_text(&cfg.performance_counter);
        self.txt_perf_threshold.set_text(&cfg.perf_counter_threshold);
        self.txt_filter_include.set_text(&cfg.exception_filter_include);
        self.txt_filter_exclude.set_text(&cfg.exception_filter_exclude);
        set_checked(&self.chk_wer, cfg.wer_integration);
        self.txt_avoid_terminate.set_text(&cfg.avoid_terminate_timeout.to_string());

        let pd_dir = std::path::Path::new(&cfg.proc_dump_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(crate::paths::install_dir);
        let choice = crate::bitness::select_binary(crate::bitness::detect(&cfg.target_name), &pd_dir, true);
        let text = match &choice.warning {
            Some(w) => format!("{} - {w}", choice.summary),
            None => choice.summary.clone(),
        };
        self.lbl_bitness.set_text(&text);

        self.suppress_custom.set(prev);
    }

    /// Always succeeds -- ProcDump never blocks navigation. Returns `bool`
    /// to match the page contract every page shares (only Notify's `save`
    /// ever returns `false`, on invalid email settings).
    pub fn save(&self, cfg: &mut Config) -> bool {
        cfg.scenario = match self.cmb_scenario.selection_string() {
            Some(s) if s != "Custom" => s,
            _ => String::new(),
        };
        cfg.proc_dump_path = self.txt_procdump_path.text().trim().to_string();
        cfg.dump_directory = self.txt_dump_dir.text().trim().to_string();
        cfg.dump_type = self.cmb_dump_type.selection_string().unwrap_or_else(|| "Full".into());
        cfg.dump_on_exception = checked(&self.chk_exception);
        cfg.dump_on_terminate = checked(&self.chk_terminate);
        cfg.hang_window_seconds = if checked(&self.chk_hang) { 1 } else { 0 };
        cfg.cpu_threshold = parse_i32(&self.txt_cpu);
        cfg.cpu_low_threshold = parse_i32(&self.txt_cpu_low);
        cfg.cpu_duration_seconds = parse_i32(&self.txt_cpu_dur);
        cfg.max_dumps = parse_i32(&self.txt_count).max(1);
        cfg.cpu_per_unit = checked(&self.chk_cpu_per_unit);
        cfg.memory_commit_mb = parse_i32(&self.txt_mem);
        cfg.use_clone = checked(&self.chk_clone);
        cfg.avoid_outage = checked(&self.chk_avoid);
        cfg.overwrite_existing = checked(&self.chk_overwrite);
        cfg.wait_for_process = checked(&self.chk_wait);
        cfg.restart_delay_seconds = parse_i32(&self.txt_restart_delay);
        cfg.min_free_disk_mb = parse_i64(&self.txt_min_disk);
        cfg.performance_counter = self.txt_perf_counter.text();
        cfg.perf_counter_threshold = self.txt_perf_threshold.text();
        cfg.exception_filter_include = self.txt_filter_include.text();
        cfg.exception_filter_exclude = self.txt_filter_exclude.text();
        cfg.wer_integration = checked(&self.chk_wer);
        cfg.avoid_terminate_timeout = parse_i32(&self.txt_avoid_terminate);
        true
    }
}

//! Advanced options dialog: the ProcDump power-user fields plus maintenance
//! settings that left the merged Monitor page. Owned, reusable window —
//! nwg hides it on WM_CLOSE; the shell's handler saves fields back into the
//! config and re-enables the main window.

use crate::config::Config;
use native_windows_gui as nwg;
use std::cell::Cell;

use super::theme;

pub struct AdvancedDialog {
    pub window: nwg::Window,
    #[allow(dead_code)]
    header_font: nwg::Font,
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    pub txt_manual_target: nwg::TextInput,

    pub chk_clone: nwg::CheckBox,
    pub chk_avoid: nwg::CheckBox,
    pub chk_overwrite: nwg::CheckBox,
    pub chk_wait: nwg::CheckBox,
    pub chk_wer: nwg::CheckBox,
    pub chk_cpu_per_unit: nwg::CheckBox,

    pub txt_restart_delay: nwg::TextInput,
    pub txt_min_disk: nwg::TextInput,
    pub txt_perf_counter: nwg::TextInput,
    pub txt_perf_threshold: nwg::TextInput,
    pub txt_filter_include: nwg::TextInput,
    pub txt_filter_exclude: nwg::TextInput,
    pub txt_avoid_terminate: nwg::TextInput,

    pub txt_log_size: nwg::TextInput,
    pub txt_log_files: nwg::TextInput,
    pub txt_ret_days: nwg::TextInput,
    pub txt_ret_gb: nwg::TextInput,
    pub txt_stab_timeout: nwg::TextInput,

    pub btn_close: nwg::Button,
    /// Any edit inside the dialog (shell handler sets this; Monitor page
    /// flips the scenario to Custom when it comes back true).
    pub dirty: Cell<bool>,
}

fn mk_label(parent: &nwg::Window, text: &str, pos: (i32, i32), size: (i32, i32)) -> nwg::Label {
    let mut l = nwg::Label::default();
    nwg::Label::builder().text(text).position(pos).size(size).parent(parent).build(&mut l).unwrap();
    l
}

fn mk_text(parent: &nwg::Window, pos: (i32, i32), size: (i32, i32)) -> nwg::TextInput {
    let mut t = nwg::TextInput::default();
    nwg::TextInput::builder().position(pos).size(size).parent(parent).build(&mut t).unwrap();
    t
}

fn mk_check(parent: &nwg::Window, text: &str, pos: (i32, i32), size: (i32, i32)) -> nwg::CheckBox {
    let mut c = nwg::CheckBox::default();
    nwg::CheckBox::builder().text(text).position(pos).size(size).parent(parent).build(&mut c).unwrap();
    c
}

fn checked(c: &nwg::CheckBox) -> bool {
    c.check_state() == nwg::CheckBoxState::Checked
}

fn set_checked(c: &nwg::CheckBox, v: bool) {
    c.set_check_state(if v { nwg::CheckBoxState::Checked } else { nwg::CheckBoxState::Unchecked });
}

fn parse_i32(t: &nwg::TextInput) -> i32 {
    t.text().trim().parse().unwrap_or(0)
}

pub fn build(owner: &nwg::Window) -> AdvancedDialog {
    let header_font = theme::semibold(15);
    let mut captions: Vec<nwg::Label> = Vec::new();

    let mut window = nwg::Window::default();
    nwg::Window::builder()
        .size((560, 520))
        .center(true)
        .title("Advanced options")
        .flags(nwg::WindowFlags::WINDOW) // built hidden; shown on demand
        .parent(Some(owner))
        .build(&mut window)
        .expect("advanced dialog");
    theme::attach(&window.handle);

    const PAD: i32 = 24;
    const FX: i32 = 210; // label col 24..204, field col from 210
    let mut y = 20;

    captions.push({
        let l = mk_label(&window, "Target", (PAD, y), (500, 20));
        l.set_font(Some(&header_font));
        l
    });
    y += 28;
    captions.push(mk_label(&window, "Name override (not in list):", (PAD, y), (182, 20)));
    let txt_manual_target = mk_text(&window, (FX, y - 2), (326, 26));
    y += 30;
    captions.push({
        let l = mk_label(
            &window,
            "For a process that isn't running yet (used with -w wait for launch).",
            (PAD, y),
            (512, 16),
        );
        theme::register_muted(&l.handle);
        l
    });
    y += 28;

    captions.push({
        let l = mk_label(&window, "ProcDump switches", (PAD, y), (500, 20));
        l.set_font(Some(&header_font));
        l
    });
    y += 28;
    let chk_clone = mk_check(&window, "-r clone", (PAD, y), (120, 22));
    let chk_avoid = mk_check(&window, "-a avoid outage", (194, y), (150, 22));
    let chk_overwrite = mk_check(&window, "-o overwrite", (384, y), (140, 22));
    y += 28;
    let chk_wait = mk_check(&window, "-w wait for launch", (PAD, y), (150, 22));
    let chk_wer = mk_check(&window, "-wer WER integration", (194, y), (170, 22));
    let chk_cpu_per_unit = mk_check(&window, "-u per-CPU", (384, y), (120, 22));
    y += 34;

    captions.push(mk_label(&window, "Restart delay (s):", (PAD, y), (182, 20)));
    let txt_restart_delay = mk_text(&window, (FX, y - 2), (60, 26));
    captions.push(mk_label(&window, "Min free disk MB:", (290, y), (130, 20)));
    let txt_min_disk = mk_text(&window, (426, y - 2), (80, 26));
    y += 34;

    captions.push(mk_label(&window, "Perf counter (-p):", (PAD, y), (182, 20)));
    let txt_perf_counter = mk_text(&window, (FX, y - 2), (170, 26));
    captions.push(mk_label(&window, "Threshold (-pl):", (390, y), (110, 20)));
    let txt_perf_threshold = mk_text(&window, (506, y - 2), (36, 26));
    y += 34;

    captions.push(mk_label(&window, "Exception filter (-f):", (PAD, y), (182, 20)));
    let txt_filter_include = mk_text(&window, (FX, y - 2), (170, 26));
    captions.push(mk_label(&window, "Exclude (-fx):", (390, y), (110, 20)));
    let txt_filter_exclude = mk_text(&window, (506, y - 2), (36, 26));
    y += 34;

    captions.push(mk_label(&window, "Avoid-terminate (-at s):", (PAD, y), (182, 20)));
    let txt_avoid_terminate = mk_text(&window, (FX, y - 2), (60, 26));
    y += 40;

    captions.push({
        let l = mk_label(&window, "Logs && retention", (PAD, y), (500, 20));
        l.set_font(Some(&header_font));
        l
    });
    y += 28;
    captions.push(mk_label(&window, "Max log size (MB):", (PAD, y), (182, 20)));
    let txt_log_size = mk_text(&window, (FX, y - 2), (60, 26));
    captions.push(mk_label(&window, "Max log files:", (290, y), (110, 20)));
    let txt_log_files = mk_text(&window, (426, y - 2), (60, 26));
    y += 34;
    captions.push(mk_label(&window, "Dump retention (days):", (PAD, y), (182, 20)));
    let txt_ret_days = mk_text(&window, (FX, y - 2), (60, 26));
    captions.push(mk_label(&window, "Max size (GB):", (290, y), (110, 20)));
    let txt_ret_gb = mk_text(&window, (426, y - 2), (60, 26));
    y += 34;
    captions.push(mk_label(&window, "Stability timeout (s):", (PAD, y), (182, 20)));
    let txt_stab_timeout = mk_text(&window, (FX, y - 2), (60, 26));

    let mut btn_close = nwg::Button::default();
    nwg::Button::builder()
        .text("Save && Close")
        .position((406, 468))
        .size((130, 32))
        .parent(&window)
        .build(&mut btn_close)
        .unwrap();

    AdvancedDialog {
        window,
        header_font,
        captions,
        txt_manual_target,
        chk_clone,
        chk_avoid,
        chk_overwrite,
        chk_wait,
        chk_wer,
        chk_cpu_per_unit,
        txt_restart_delay,
        txt_min_disk,
        txt_perf_counter,
        txt_perf_threshold,
        txt_filter_include,
        txt_filter_exclude,
        txt_avoid_terminate,
        txt_log_size,
        txt_log_files,
        txt_ret_days,
        txt_ret_gb,
        txt_stab_timeout,
        btn_close,
        dirty: Cell::new(false),
    }
}

impl AdvancedDialog {
    pub fn open(&self, cfg: &Config, manual_target: &str) {
        self.txt_manual_target.set_text(manual_target);
        set_checked(&self.chk_clone, cfg.use_clone);
        set_checked(&self.chk_avoid, cfg.avoid_outage);
        set_checked(&self.chk_overwrite, cfg.overwrite_existing);
        set_checked(&self.chk_wait, cfg.wait_for_process);
        set_checked(&self.chk_wer, cfg.wer_integration);
        set_checked(&self.chk_cpu_per_unit, cfg.cpu_per_unit);
        self.txt_restart_delay.set_text(&cfg.restart_delay_seconds.to_string());
        self.txt_min_disk.set_text(&cfg.min_free_disk_mb.to_string());
        self.txt_perf_counter.set_text(&cfg.performance_counter);
        self.txt_perf_threshold.set_text(&cfg.perf_counter_threshold);
        self.txt_filter_include.set_text(&cfg.exception_filter_include);
        self.txt_filter_exclude.set_text(&cfg.exception_filter_exclude);
        self.txt_avoid_terminate.set_text(&cfg.avoid_terminate_timeout.to_string());
        self.txt_log_size.set_text(&cfg.max_log_size_mb.to_string());
        self.txt_log_files.set_text(&cfg.max_log_files.to_string());
        self.txt_ret_days.set_text(&cfg.dump_retention_days.to_string());
        self.txt_ret_gb.set_text(&cfg.dump_retention_max_gb.to_string());
        self.txt_stab_timeout.set_text(&cfg.dump_stability_timeout_seconds.to_string());
        self.dirty.set(false);
        self.window.set_visible(true);
        self.window.set_focus();
    }

    /// Writes fields into cfg; returns the manual-target override text.
    pub fn save(&self, cfg: &mut Config) -> String {
        cfg.use_clone = checked(&self.chk_clone);
        cfg.avoid_outage = checked(&self.chk_avoid);
        cfg.overwrite_existing = checked(&self.chk_overwrite);
        cfg.wait_for_process = checked(&self.chk_wait);
        cfg.wer_integration = checked(&self.chk_wer);
        cfg.cpu_per_unit = checked(&self.chk_cpu_per_unit);
        cfg.restart_delay_seconds = parse_i32(&self.txt_restart_delay);
        cfg.min_free_disk_mb = self.txt_min_disk.text().trim().parse().unwrap_or(0);
        cfg.performance_counter = self.txt_perf_counter.text();
        cfg.perf_counter_threshold = self.txt_perf_threshold.text();
        cfg.exception_filter_include = self.txt_filter_include.text();
        cfg.exception_filter_exclude = self.txt_filter_exclude.text();
        cfg.avoid_terminate_timeout = parse_i32(&self.txt_avoid_terminate);
        cfg.max_log_size_mb = parse_i32(&self.txt_log_size);
        cfg.max_log_files = parse_i32(&self.txt_log_files);
        cfg.dump_retention_days = parse_i32(&self.txt_ret_days);
        cfg.dump_retention_max_gb = self.txt_ret_gb.text().trim().parse().unwrap_or(0.0);
        cfg.dump_stability_timeout_seconds = parse_i32(&self.txt_stab_timeout);
        self.txt_manual_target.text().trim().to_string()
    }
}

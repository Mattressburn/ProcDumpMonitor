//! System Health page — port of the PS1's "System Health" tab (uptime +
//! process/service snapshots with the same default match patterns).

use crate::collect::{self, syshealth};
use native_windows_gui as nwg;
use std::rc::Rc;

use super::collect_runner::{self, CollectRunner};
use super::theme;

pub struct SysHealthPage {
    #[allow(dead_code)]
    header_font: nwg::Font,
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    pub chk_uptime: nwg::CheckBox,
    pub chk_procs: nwg::CheckBox,
    pub chk_svcs: nwg::CheckBox,
    pub txt_proc_patterns: nwg::TextInput,
    pub txt_svc_patterns: nwg::TextInput,

    pub txt_save: nwg::TextInput,
    pub btn_start: nwg::Button,
    pub btn_open: nwg::Button,
    pub lbl_status: nwg::Label,
    pub lst_progress: nwg::ListBox<String>,
    pub runner: CollectRunner,
}

fn mk_label(parent: &nwg::Frame, text: &str, pos: (i32, i32), size: (i32, i32)) -> nwg::Label {
    let mut l = nwg::Label::default();
    nwg::Label::builder().text(text).position(pos).size(size).parent(parent).build(&mut l).unwrap();
    l
}

fn mk_text(parent: &nwg::Frame, pos: (i32, i32), size: (i32, i32)) -> nwg::TextInput {
    let mut t = nwg::TextInput::default();
    nwg::TextInput::builder().position(pos).size(size).parent(parent).build(&mut t).unwrap();
    t
}

fn mk_check(
    parent: &nwg::Frame,
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

fn mk_button(parent: &nwg::Frame, text: &str, pos: (i32, i32), size: (i32, i32)) -> nwg::Button {
    let mut b = nwg::Button::default();
    nwg::Button::builder().text(text).position(pos).size(size).parent(parent).build(&mut b).unwrap();
    b
}

fn checked(c: &nwg::CheckBox) -> bool {
    c.check_state() == nwg::CheckBoxState::Checked
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> SysHealthPage {
    let header_font = theme::semibold(15);
    let mut captions: Vec<nwg::Label> = Vec::new();

    const PAD: i32 = 32;

    captions.push({
        let l = mk_label(parent, "CCURE system health snapshot", (PAD, 12), (400, 20));
        l.set_font(Some(&header_font));
        l
    });

    let chk_uptime = mk_check(parent, "OS uptime and last boot time", (PAD, 44), (400, 22), true);
    let chk_procs = mk_check(
        parent,
        "Process snapshot (CPU / memory / threads / handles / versions)",
        (PAD, 70),
        (500, 22),
        true,
    );
    let chk_svcs = mk_check(
        parent,
        "Service snapshot (state / start mode / path / dependencies)",
        (PAD, 96),
        (500, 22),
        true,
    );

    captions.push(mk_label(parent, "Process match patterns:", (PAD, 130), (190, 20)));
    let txt_proc_patterns = mk_text(parent, (232, 128), (416, 26));
    txt_proc_patterns.set_text(syshealth::DEFAULT_PATTERNS);

    captions.push(mk_label(parent, "Service match patterns:", (PAD, 164), (190, 20)));
    let txt_svc_patterns = mk_text(parent, (232, 162), (416, 26));
    txt_svc_patterns.set_text(syshealth::DEFAULT_PATTERNS);

    captions.push({
        let l = mk_label(
            parent,
            "Comma-separated substrings; empty = include everything.",
            (232, 192),
            (416, 16),
        );
        theme::register_muted(&l.handle);
        l
    });

    captions.push(mk_label(parent, "Save path (blank = Desktop):", (PAD, 222), (220, 20)));
    let txt_save = mk_text(parent, (260, 220), (278, 26));

    let btn_start = mk_button(parent, "Collect system health", (PAD, 260), (180, 32));
    let btn_open = mk_button(parent, "Open last output", (222, 260), (150, 32));
    let lbl_status = mk_label(parent, "Status: Idle", (386, 266), (262, 20));
    theme::register_muted(&lbl_status.handle);

    let mut lst_progress = nwg::ListBox::default();
    nwg::ListBox::builder()
        .position((PAD, 304))
        .size((616, 282))
        .parent(parent)
        .build(&mut lst_progress)
        .unwrap();

    let runner = collect_runner::build(parent);

    SysHealthPage {
        header_font,
        captions,
        chk_uptime,
        chk_procs,
        chk_svcs,
        txt_proc_patterns,
        txt_svc_patterns,
        txt_save,
        btn_start,
        btn_open,
        lbl_status,
        lst_progress,
        runner,
    }
}

impl SysHealthPage {
    pub fn start(&self) {
        if self.runner.running.get() {
            return;
        }
        let base = super::page_datacoll::resolve_base(&self.txt_save.text());
        let opts = syshealth::Options {
            uptime: checked(&self.chk_uptime),
            processes: checked(&self.chk_procs),
            services: checked(&self.chk_svcs),
            proc_patterns: self.txt_proc_patterns.text(),
            svc_patterns: self.txt_svc_patterns.text(),
        };
        let started = self.runner.start(move |progress| {
            let mut ctx = collect::RunContext::start(&base, progress)
                .map_err(|e| format!("cannot create run folder: {e}"))?;
            syshealth::run(&mut ctx, &opts);
            Ok(ctx.finish())
        });
        if started {
            self.lst_progress.clear();
            self.lbl_status.set_text("Status: Running...");
            self.btn_start.set_enabled(false);
        }
    }

    pub fn on_notice(&self) {
        super::page_datacoll::pump(&self.runner, &self.lst_progress, &self.lbl_status, &self.btn_start);
    }
}

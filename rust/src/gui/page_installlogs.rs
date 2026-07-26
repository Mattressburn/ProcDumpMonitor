//! Install Logs page — port of the PS1's "Install Logs Collection" tab.

use crate::collect::{self, installlogs};
use native_windows_gui as nwg;
use std::path::PathBuf;
use std::rc::Rc;

use super::collect_runner::{self, CollectRunner};
use super::theme;

pub struct InstallLogsPage {
    #[allow(dead_code)]
    header_font: nwg::Font,
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    pub txt_history: nwg::TextInput,
    pub btn_browse: nwg::Button,
    pub chk_auto: nwg::CheckBox,
    pub chk_installer_temp: nwg::CheckBox,
    pub chk_install_cache: nwg::CheckBox,

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

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> InstallLogsPage {
    let header_font = theme::semibold(15);
    let mut captions: Vec<nwg::Label> = Vec::new();

    const PAD: i32 = 32;

    captions.push({
        let l = mk_label(parent, "Extract install logs", (PAD, 12), (400, 20));
        l.set_font(Some(&header_font));
        l
    });

    captions.push(mk_label(parent, "InstallHistory.xml path:", (PAD, 44), (190, 20)));
    let txt_history = mk_text(parent, (232, 42), (306, 26));
    let btn_browse = mk_button(parent, "Browse...", (548, 40), (100, 30));

    let chk_auto = mk_check(
        parent,
        "Auto-discover InstallHistory.xml under ProgramData (Tyco/JCI)",
        (232, 76),
        (416, 22),
        true,
    );
    let chk_installer_temp = mk_check(
        parent,
        "Include InstallerTemp folder contents (logs, configs)",
        (232, 102),
        (416, 22),
        true,
    );
    let chk_install_cache = mk_check(
        parent,
        "Include InstallCache (ProgramData JCI/Tyco)",
        (232, 128),
        (416, 22),
        true,
    );

    captions.push(mk_label(parent, "Save path (blank = Desktop):", (PAD, 164), (220, 20)));
    let txt_save = mk_text(parent, (260, 162), (278, 26));

    let btn_start = mk_button(parent, "Start extraction", (PAD, 204), (150, 32));
    let btn_open = mk_button(parent, "Open last output", (192, 204), (150, 32));
    let lbl_status = mk_label(parent, "Status: Idle", (356, 210), (292, 20));
    theme::register_muted(&lbl_status.handle);

    let mut lst_progress = nwg::ListBox::default();
    nwg::ListBox::builder()
        .position((PAD, 248))
        .size((616, 338))
        .parent(parent)
        .build(&mut lst_progress)
        .unwrap();

    let runner = collect_runner::build(parent);

    InstallLogsPage {
        header_font,
        captions,
        txt_history,
        btn_browse,
        chk_auto,
        chk_installer_temp,
        chk_install_cache,
        txt_save,
        btn_start,
        btn_open,
        lbl_status,
        lst_progress,
        runner,
    }
}

impl InstallLogsPage {
    pub fn browse_history(&self, parent: nwg::ControlHandle) {
        let mut dialog = nwg::FileDialog::default();
        let built = nwg::FileDialog::builder()
            .title("Select InstallHistory.xml")
            .action(nwg::FileDialogAction::Open)
            .filters("XML(*.xml)|All(*.*)")
            .build(&mut dialog)
            .is_ok();
        if built && dialog.run(Some(parent)) {
            if let Ok(p) = dialog.get_selected_item() {
                self.txt_history.set_text(&p.to_string_lossy());
            }
        }
    }

    pub fn start(&self) {
        if self.runner.running.get() {
            return;
        }
        let base = super::page_datacoll::resolve_base(&self.txt_save.text());
        let history = self.txt_history.text().trim().to_string();
        let opts = installlogs::Options {
            history_path: if history.is_empty() { None } else { Some(PathBuf::from(history)) },
            auto_discover: checked(&self.chk_auto),
            include_installer_temp: checked(&self.chk_installer_temp),
            include_install_cache: checked(&self.chk_install_cache),
        };
        let started = self.runner.start(move |progress| {
            let mut ctx = collect::RunContext::start(&base, progress)
                .map_err(|e| format!("cannot create run folder: {e}"))?;
            installlogs::run(&mut ctx, &opts);
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

//! SMTP settings dialog: full email configuration that left the merged
//! Monitor page (which keeps only the enable toggle + To list). Same
//! DPAPI-secret handling as the old Notify page: passwords never round-trip
//! as plaintext; a cue banner marks an existing saved blob.

use crate::config::Config;
use crate::notify;
use native_windows_gui as nwg;
use std::cell::Cell;

use super::theme;

const UNCHANGED: &str = "(unchanged)";

pub struct SmtpDialog {
    pub window: nwg::Window,
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    pub txt_smtp: nwg::TextInput,
    pub txt_port: nwg::TextInput,
    pub chk_ssl: nwg::CheckBox,
    pub txt_from: nwg::TextInput,
    pub txt_cc: nwg::TextInput,
    pub txt_user: nwg::TextInput,
    pub txt_password: nwg::TextInput,
    pub btn_validate: nwg::Button,
    pub btn_test: nwg::Button,
    pub lbl_status: nwg::Label,
    pub btn_close: nwg::Button,
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

pub fn build(owner: &nwg::Window) -> SmtpDialog {
    let mut captions: Vec<nwg::Label> = Vec::new();

    // +NC height (see dlg_advanced note): parented windows aren't
    // AdjustWindowRectEx'd, so pad for the title bar or the bottom button
    // clips off the client area.
    let mut window = nwg::Window::default();
    nwg::Window::builder()
        .size((520, 366))
        .center(true)
        .title("SMTP settings")
        .flags(nwg::WindowFlags::WINDOW)
        .parent(Some(owner))
        .build(&mut window)
        .expect("smtp dialog");
    theme::attach(&window.handle);

    const PAD: i32 = 24;
    const FX: i32 = 170;

    captions.push(mk_label(&window, "SMTP server:", (PAD, 22), (140, 20)));
    let txt_smtp = mk_text(&window, (FX, 20), (200, 26));
    captions.push(mk_label(&window, "Port:", (382, 22), (40, 20)));
    let txt_port = mk_text(&window, (426, 20), (48, 26));

    let mut chk_ssl = nwg::CheckBox::default();
    nwg::CheckBox::builder()
        .text("Use SSL/TLS")
        .position((FX, 54))
        .size((120, 22))
        .parent(&window)
        .build(&mut chk_ssl)
        .unwrap();

    captions.push(mk_label(&window, "From address:", (PAD, 88), (140, 20)));
    let txt_from = mk_text(&window, (FX, 86), (200, 26));

    captions.push(mk_label(&window, "CC (; separated):", (PAD, 122), (140, 20)));
    let txt_cc = mk_text(&window, (FX, 120), (326, 26));

    captions.push(mk_label(&window, "SMTP username:", (PAD, 156), (140, 20)));
    let txt_user = mk_text(&window, (FX, 154), (200, 26));

    captions.push(mk_label(&window, "SMTP password:", (PAD, 190), (140, 20)));
    let mut txt_password = nwg::TextInput::default();
    nwg::TextInput::builder()
        .position((FX, 188))
        .size((200, 26))
        .password(Some('\u{2022}'))
        .parent(&window)
        .build(&mut txt_password)
        .unwrap();

    let mut btn_validate = nwg::Button::default();
    nwg::Button::builder()
        .text("Validate SMTP")
        .position((PAD, 228))
        .size((130, 30))
        .parent(&window)
        .build(&mut btn_validate)
        .unwrap();
    let mut btn_test = nwg::Button::default();
    nwg::Button::builder()
        .text("Send Test Email")
        .position((162, 228))
        .size((150, 30))
        .parent(&window)
        .build(&mut btn_test)
        .unwrap();

    let lbl_status = mk_label(&window, "", (PAD, 266), (472, 20));
    theme::register_muted(&lbl_status.handle);

    let mut btn_close = nwg::Button::default();
    nwg::Button::builder()
        .text("Save && Close")
        .position((366, 288))
        .size((130, 32))
        .parent(&window)
        .build(&mut btn_close)
        .unwrap();

    SmtpDialog {
        window,
        captions,
        txt_smtp,
        txt_port,
        chk_ssl,
        txt_from,
        txt_cc,
        txt_user,
        txt_password,
        btn_validate,
        btn_test,
        lbl_status,
        btn_close,
        dirty: Cell::new(false),
    }
}

fn checked(c: &nwg::CheckBox) -> bool {
    c.check_state() == nwg::CheckBoxState::Checked
}

impl SmtpDialog {
    pub fn open(&self, cfg: &Config) {
        self.txt_smtp.set_text(&cfg.smtp_server);
        self.txt_port.set_text(&cfg.smtp_port.to_string());
        self.chk_ssl.set_check_state(if cfg.use_ssl {
            nwg::CheckBoxState::Checked
        } else {
            nwg::CheckBoxState::Unchecked
        });
        self.txt_from.set_text(&cfg.from_address);
        self.txt_cc.set_text(&cfg.cc_address);
        self.txt_user.set_text(&cfg.smtp_username);
        self.txt_password.set_text("");
        self.txt_password.set_placeholder_text(
            if cfg.encrypted_password_blob.is_empty() { None } else { Some(UNCHANGED) },
        );
        self.lbl_status.set_text("");
        self.dirty.set(false);
        self.window.set_visible(true);
        self.window.set_focus();
    }

    /// Field values -> cfg (plain fields; password only when freshly typed).
    pub fn save(&self, cfg: &mut Config) {
        cfg.smtp_server = self.txt_smtp.text().trim().to_string();
        cfg.smtp_port = self.txt_port.text().trim().parse().unwrap_or(25);
        cfg.use_ssl = checked(&self.chk_ssl);
        cfg.from_address = self.txt_from.text().trim().to_string();
        cfg.cc_address = self.txt_cc.text().trim().to_string();
        cfg.smtp_username = self.txt_user.text().trim().to_string();
        let pw = self.txt_password.text();
        if !pw.is_empty() {
            cfg.encrypted_password_blob =
                crate::secrets::protect(&pw, crate::secrets::SMTP_ENTROPY);
            // NOT cleared here: send_test_email() saves into a throwaway
            // clone and the user may still Save && Close afterwards. The
            // shell's close flow clears the field once the real save runs.
        }
    }

    pub fn validate_smtp(&self) {
        let server = self.txt_smtp.text().trim().to_string();
        let port: u16 = self.txt_port.text().trim().parse().unwrap_or(25);
        let (ok, msg) = notify::validate_smtp_connectivity(&server, port, 5000);
        self.lbl_status
            .set_text(&format!("{} {msg}", if ok { "OK:" } else { "ERROR:" }));
    }

    /// Test email from a throwaway config clone: current dialog fields + the
    /// Monitor page's To list already saved in cfg. Never mutates state.cfg.
    pub fn send_test_email(&self, cfg_snapshot: &Config) {
        let mut clone = cfg_snapshot.clone();
        self.save(&mut clone);
        clone.email_enabled = true;
        match notify::send_test_email(&clone) {
            Ok(()) => self.lbl_status.set_text("OK: Test email sent."),
            Err(e) => self.lbl_status.set_text(&format!("ERROR: Test email failed: {e}")),
        }
    }
}

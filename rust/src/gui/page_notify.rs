use crate::config::Config;
use crate::notify;
use native_windows_gui as nwg;
use std::rc::Rc;

/// Cue-banner text shown (via the real win32 EM_SETCUEBANNER placeholder, not
/// literal control text) when a DPAPI blob already exists but the field
/// itself is empty. Because it's a placeholder, `.text()` still reads "" --
/// save()'s "empty means keep the existing blob" check needs no special-case
/// guard against it.
const UNCHANGED: &str = "(unchanged)";

pub struct NotifyPage {
    #[allow(dead_code)]
    bold_font: nwg::Font,
    #[allow(dead_code)]
    captions: Vec<nwg::Label>,

    pub chk_email: nwg::CheckBox,
    pub txt_smtp: nwg::TextInput,
    pub txt_port: nwg::TextInput,
    pub chk_ssl: nwg::CheckBox,
    pub txt_from: nwg::TextInput,
    pub txt_to: nwg::TextInput,
    pub txt_cc: nwg::TextInput,
    pub txt_user: nwg::TextInput,
    pub txt_password: nwg::TextInput,
    pub btn_validate: nwg::Button,
    pub btn_test_email: nwg::Button,

    pub chk_webhook: nwg::CheckBox,
    pub txt_webhook: nwg::TextInput,

    pub txt_log_size: nwg::TextInput,
    pub txt_log_files: nwg::TextInput,
    pub txt_ret_days: nwg::TextInput,
    pub txt_ret_gb: nwg::TextInput,
    pub txt_stab_timeout: nwg::TextInput,

    pub lbl_notify_status: nwg::Label,
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

fn mk_text<P: Into<nwg::ControlHandle> + Copy>(parent: P, pos: (i32, i32), size: (i32, i32)) -> nwg::TextInput {
    let mut t = nwg::TextInput::default();
    nwg::TextInput::builder().position(pos).size(size).parent(parent).build(&mut t).unwrap();
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
    t.text().trim().parse().unwrap_or(0)
}

fn parse_f64(t: &nwg::TextInput) -> f64 {
    t.text().trim().parse().unwrap_or(0.0)
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> NotifyPage {
    let mut bold_font = nwg::Font::default();
    let _ = nwg::Font::builder().family("Segoe UI").weight(700).build(&mut bold_font);

    let mut captions: Vec<nwg::Label> = Vec::new();

    let chk_email = mk_check(parent, "Enable email notifications", (10, 8), (260, 22));

    captions.push(mk_label(parent, "SMTP server:", (10, 42), (110, 22)));
    let txt_smtp = mk_text(parent, (130, 40), (260, 24));
    captions.push(mk_label(parent, "Port:", (400, 42), (40, 22)));
    let txt_port = mk_text(parent, (440, 40), (60, 24));
    let chk_ssl = mk_check(parent, "Use SSL/TLS", (520, 42), (140, 22));

    captions.push(mk_label(parent, "From address:", (10, 74), (110, 22)));
    let txt_from = mk_text(parent, (130, 72), (600, 24));

    captions.push(mk_label(parent, "To (semicolon-separated):", (10, 106), (190, 22)));
    let txt_to = mk_text(parent, (200, 104), (540, 24));

    captions.push(mk_label(parent, "CC (semicolon-separated):", (10, 138), (190, 22)));
    let txt_cc = mk_text(parent, (200, 136), (540, 24));

    captions.push(mk_label(parent, "SMTP username:", (10, 170), (110, 22)));
    let txt_user = mk_text(parent, (130, 168), (260, 24));
    captions.push(mk_label(parent, "SMTP password:", (410, 170), (110, 22)));
    let mut txt_password = nwg::TextInput::default();
    nwg::TextInput::builder()
        .position((520, 168))
        .size((220, 24))
        .password(Some('\u{2022}'))
        .parent(parent)
        .build(&mut txt_password)
        .unwrap();

    let btn_validate = mk_button(parent, "Validate SMTP", (10, 202), (150, 28));
    let btn_test_email = mk_button(parent, "Send Test Email", (170, 202), (150, 28));

    captions.push({
        let l = mk_label(parent, "Webhook notifications", (10, 244), (300, 22));
        l.set_font(Some(&bold_font));
        l
    });
    let chk_webhook = mk_check(parent, "Enable webhook notifications", (10, 268), (280, 22));
    captions.push(mk_label(parent, "Webhook URL:", (10, 300), (110, 22)));
    let txt_webhook = mk_text(parent, (130, 298), (610, 24));

    captions.push({
        let l = mk_label(parent, "Maintenance", (10, 334), (300, 22));
        l.set_font(Some(&bold_font));
        l
    });

    captions.push(mk_label(parent, "Max log size (MB):", (10, 360), (150, 22)));
    let txt_log_size = mk_text(parent, (170, 358), (60, 24));
    captions.push(mk_label(parent, "Max log files:", (250, 360), (110, 22)));
    let txt_log_files = mk_text(parent, (360, 358), (60, 24));
    captions.push(mk_label(parent, "Dump retention (days):", (440, 360), (170, 22)));
    let txt_ret_days = mk_text(parent, (620, 358), (60, 24));

    captions.push(mk_label(parent, "Dump retention max (GB):", (10, 392), (190, 22)));
    let txt_ret_gb = mk_text(parent, (200, 390), (60, 24));
    captions.push(mk_label(parent, "Dump stability timeout (s):", (280, 392), (190, 22)));
    let txt_stab_timeout = mk_text(parent, (470, 390), (60, 24));

    let mut lbl_notify_status = nwg::Label::default();
    nwg::Label::builder()
        .text("")
        .position((10, 424))
        .size((720, 40))
        .parent(parent)
        .build(&mut lbl_notify_status)
        .unwrap();

    NotifyPage {
        bold_font,
        captions,
        chk_email,
        txt_smtp,
        txt_port,
        chk_ssl,
        txt_from,
        txt_to,
        txt_cc,
        txt_user,
        txt_password,
        btn_validate,
        btn_test_email,
        chk_webhook,
        txt_webhook,
        txt_log_size,
        txt_log_files,
        txt_ret_days,
        txt_ret_gb,
        txt_stab_timeout,
        lbl_notify_status,
    }
}

impl NotifyPage {
    fn set_email_group_enabled(&self, v: bool) {
        self.txt_smtp.set_enabled(v);
        self.txt_port.set_enabled(v);
        self.chk_ssl.set_enabled(v);
        self.txt_from.set_enabled(v);
        self.txt_to.set_enabled(v);
        self.txt_cc.set_enabled(v);
        self.txt_user.set_enabled(v);
        self.txt_password.set_enabled(v);
        self.btn_validate.set_enabled(v);
        self.btn_test_email.set_enabled(v);
    }

    /// Wired to chk_email's OnButtonClick.
    pub fn on_email_toggled(&self) {
        self.set_email_group_enabled(checked(&self.chk_email));
    }

    /// Wired to chk_webhook's OnButtonClick.
    pub fn on_webhook_toggled(&self) {
        self.txt_webhook.set_enabled(checked(&self.chk_webhook));
    }

    fn set_status(&self, ok: bool, msg: &str) {
        self.lbl_notify_status.set_text(&format!("{} {msg}", if ok { "OK:" } else { "ERROR:" }));
    }

    /// Wired to btn_validate's OnButtonClick.
    pub fn validate_smtp(&self) {
        let server = self.txt_smtp.text().trim().to_string();
        let port: u16 = self.txt_port.text().trim().parse().unwrap_or(25);
        let (ok, msg) = notify::validate_smtp_connectivity(&server, port, 5000);
        self.set_status(ok, &msg);
    }

    /// Wired to btn_test_email's OnButtonClick. Sends from a throwaway clone
    /// of the in-progress config (current field text + a protected copy of
    /// any freshly typed password/webhook URL) -- never mutates `state.cfg`
    /// and never clears the password field, unlike save().
    pub fn send_test_email(&self, state: &super::WizardState) {
        let mut clone = state.cfg.borrow().clone();
        self.apply_fields(&mut clone);
        self.protect_into(&mut clone);
        match notify::send_test_email(&clone) {
            Ok(()) => self.set_status(true, "Test email sent."),
            Err(e) => self.set_status(false, &format!("Test email failed: {e}")),
        }
    }

    pub fn load(&self, cfg: &Config) {
        set_checked(&self.chk_email, cfg.email_enabled);
        self.txt_smtp.set_text(&cfg.smtp_server);
        self.txt_port.set_text(&cfg.smtp_port.to_string());
        set_checked(&self.chk_ssl, cfg.use_ssl);
        self.txt_from.set_text(&cfg.from_address);
        self.txt_to.set_text(&cfg.to_address);
        self.txt_cc.set_text(&cfg.cc_address);
        self.txt_user.set_text(&cfg.smtp_username);
        self.set_email_group_enabled(cfg.email_enabled);

        // Never populate the real password text with decrypted plaintext --
        // a cue banner (not real content) hints that a saved value exists.
        self.txt_password.set_text("");
        self.txt_password
            .set_placeholder_text(if cfg.encrypted_password_blob.is_empty() { None } else { Some(UNCHANGED) });

        set_checked(&self.chk_webhook, cfg.webhook_enabled);
        self.txt_webhook.set_text("");
        self.txt_webhook.set_placeholder_text(
            if cfg.encrypted_webhook_url_blob.is_empty() { None } else { Some(UNCHANGED) },
        );
        self.txt_webhook.set_enabled(cfg.webhook_enabled);

        self.txt_log_size.set_text(&cfg.max_log_size_mb.to_string());
        self.txt_log_files.set_text(&cfg.max_log_files.to_string());
        self.txt_ret_days.set_text(&cfg.dump_retention_days.to_string());
        self.txt_ret_gb.set_text(&cfg.dump_retention_max_gb.to_string());
        self.txt_stab_timeout.set_text(&cfg.dump_stability_timeout_seconds.to_string());

        self.lbl_notify_status.set_text("");
    }

    /// Copies every plain (non-DPAPI) field into `cfg`. Shared by `save()`
    /// and `send_test_email()`'s throwaway clone. Never touches
    /// `encrypted_password_blob` / `encrypted_webhook_url_blob` -- callers
    /// that want blob updates call `protect_into` too.
    fn apply_fields(&self, cfg: &mut Config) {
        cfg.email_enabled = checked(&self.chk_email);
        cfg.smtp_server = self.txt_smtp.text().trim().to_string();
        cfg.smtp_port = self.txt_port.text().trim().parse().unwrap_or(25);
        cfg.use_ssl = checked(&self.chk_ssl);
        cfg.from_address = self.txt_from.text().trim().to_string();
        cfg.to_address = self.txt_to.text().trim().to_string();
        cfg.cc_address = self.txt_cc.text().trim().to_string();
        cfg.smtp_username = self.txt_user.text().trim().to_string();
        cfg.webhook_enabled = checked(&self.chk_webhook);

        cfg.max_log_size_mb = parse_i32(&self.txt_log_size);
        cfg.max_log_files = parse_i32(&self.txt_log_files);
        cfg.dump_retention_days = parse_i32(&self.txt_ret_days);
        cfg.dump_retention_max_gb = parse_f64(&self.txt_ret_gb);
        cfg.dump_stability_timeout_seconds = parse_i32(&self.txt_stab_timeout);
    }

    /// DPAPI-protects the password/webhook-url fields into `cfg`'s blobs
    /// *only* when the field currently holds freshly typed text (a cue
    /// banner reads back as "" so it never gets re-encrypted). Leaves the
    /// existing blob -- and the on-screen field -- untouched otherwise.
    fn protect_into(&self, cfg: &mut Config) {
        let pw = self.txt_password.text();
        if !pw.is_empty() {
            cfg.encrypted_password_blob = crate::secrets::protect(&pw, crate::secrets::SMTP_ENTROPY);
        }
        let url = self.txt_webhook.text();
        if !url.is_empty() {
            cfg.encrypted_webhook_url_blob = crate::secrets::protect(&url, crate::secrets::WEBHOOK_ENTROPY);
        }
    }

    /// Port of C#'s `ValidateAddressList`, minimal: From required, at least
    /// one To, every To/CC entry contains '@'. Only enforced when email is
    /// enabled -- an unused email group is never blocking.
    pub fn save(&self, cfg: &mut Config) -> bool {
        if checked(&self.chk_email) {
            let from = self.txt_from.text().trim().to_string();
            let to_list = notify::split_addresses(&self.txt_to.text());
            let cc_list = notify::split_addresses(&self.txt_cc.text());
            let bad_addr = to_list.iter().chain(cc_list.iter()).any(|a| !a.contains('@'));
            if from.is_empty() || to_list.is_empty() || bad_addr {
                nwg::modal_error_message(
                    self.chk_email.handle,
                    "Invalid email settings",
                    "Enter a From address and at least one valid To address; every To/CC entry must contain '@'.",
                );
                return false; // validation failed before any field was written -- cfg is untouched
            }
        }

        self.apply_fields(cfg);
        self.protect_into(cfg);
        // Typed secrets never round-trip as plaintext -- clear them now that
        // they've been captured into the (possibly just-updated) blobs.
        self.txt_password.set_text("");
        self.txt_webhook.set_text("");
        true
    }
}

// ponytail: NotifyQueue::enqueue_dump/enqueue_warning (and everything they
// close over — send_email, post_webhook, etc.) are only called from
// monitor.rs, which is #[cfg(windows)] — this product's entry points are
// Windows-only.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::config::Config;
use crate::logger;
use serde::Serialize;

pub fn split_addresses(s: &str) -> Vec<String> {
    s.split(';').map(str::trim).filter(|a| !a.is_empty()).map(String::from).collect()
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TlsMode { Wrapper, Required, Opportunistic }

/// C# rule: UseSsl+465 = implicit SSL; UseSsl+other = STARTTLS; else opportunistic.
pub fn tls_mode(use_ssl: bool, port: u16) -> TlsMode {
    if use_ssl {
        if port == 465 { TlsMode::Wrapper } else { TlsMode::Required }
    } else {
        TlsMode::Opportunistic
    }
}

pub fn machine_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into())
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn dump_email(target: &str, machine: &str, dump_path: &str) -> (String, String) {
    let subject = format!("[LogDump] Dump created for {target} on {machine}");
    let body = format!(
        "A process dump was captured.\r\n\r\n\
         Target:     {target}\r\n\
         Computer:   {machine}\r\n\
         Dump File:  {dump_path}\r\n\
         Timestamp:  {}\r\n",
        timestamp()
    );
    (subject, body)
}

#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    #[serde(rename = "@type")] pub type_: String,
    pub summary: String,
    #[serde(rename = "themeColor")] pub theme_color: String,
    pub title: String,
    pub text: String,
}

pub fn webhook_payload_dump(target: &str, machine: &str, dump_path: &str) -> WebhookPayload {
    WebhookPayload {
        type_: "MessageCard".into(),
        summary: format!("Dump created for {target}"),
        theme_color: "FF0000".into(),
        title: format!("[LogDump] Dump created for {target} on {machine}"),
        text: format!(
            "**Target:** {target}\n\n**Computer:** {machine}\n\n**Dump File:** {dump_path}\n\n**Timestamp:** {}",
            timestamp()
        ),
    }
}

pub fn webhook_payload_warning(subject: &str, message: &str) -> WebhookPayload {
    WebhookPayload {
        type_: "MessageCard".into(),
        summary: subject.into(),
        theme_color: "FFAA00".into(),
        title: subject.into(),
        text: message.into(),
    }
}

/// Password seam: real DPAPI on windows (Task 7), passthrough elsewhere so
/// Linux tests never need DPAPI.
fn decrypt_password(cfg: &Config) -> String {
    #[cfg(windows)]
    { crate::secrets::unprotect(&cfg.encrypted_password_blob, crate::secrets::SMTP_ENTROPY) }
    #[cfg(not(windows))]
    { cfg.encrypted_password_blob.clone() }
}

fn effective_webhook_url(cfg: &Config) -> String {
    #[cfg(windows)]
    {
        if !cfg.encrypted_webhook_url_blob.is_empty() {
            return crate::secrets::unprotect(&cfg.encrypted_webhook_url_blob, crate::secrets::WEBHOOK_ENTROPY);
        }
    }
    cfg.webhook_url.clone()
}

pub fn send_email(cfg: &Config, subject: &str, body: &str) -> Result<(), String> {
    use lettre::message::Mailbox;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::transport::smtp::client::{Tls, TlsParameters};
    use lettre::{Message, SmtpTransport, Transport};

    let from: Mailbox = cfg.from_address.parse().map_err(|e| format!("From: {e}"))?;
    let mut msg = Message::builder().from(from).subject(subject);
    for to in split_addresses(&cfg.to_address) {
        msg = msg.to(to.parse().map_err(|e| format!("To '{to}': {e}"))?);
    }
    for cc in split_addresses(&cfg.cc_address) {
        msg = msg.cc(cc.parse().map_err(|e| format!("Cc '{cc}': {e}"))?);
    }
    let email = msg.body(body.to_string()).map_err(|e| e.to_string())?;

    let tls_params = TlsParameters::new(cfg.smtp_server.clone()).map_err(|e| e.to_string())?;
    let tls = match tls_mode(cfg.use_ssl, cfg.smtp_port) {
        TlsMode::Wrapper => Tls::Wrapper(tls_params),
        TlsMode::Required => Tls::Required(tls_params),
        TlsMode::Opportunistic => Tls::Opportunistic(tls_params),
    };
    let mut builder = SmtpTransport::builder_dangerous(&cfg.smtp_server)
        .port(cfg.smtp_port)
        .tls(tls)
        .timeout(Some(std::time::Duration::from_secs(30)));
    if !cfg.smtp_username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(
            cfg.smtp_username.clone(),
            decrypt_password(cfg),
        ));
    }
    builder.build().send(&email).map(|_| ()).map_err(|e| e.to_string())
}

// ponytail: wired up by the GUI's "Send test email" button (Task 9); no
// caller yet on either platform, unlike send_email which the monitor uses.
#[allow(dead_code)]
pub fn send_test_email(cfg: &Config) -> Result<(), String> {
    let machine = machine_name();
    let subject = format!("[LogDump] Test email from {machine}");
    let body = format!(
        "This is a test email from LogDump.\r\n\r\n\
         Computer:   {machine}\r\n\
         Timestamp:  {}\r\n",
        timestamp()
    );
    send_email(cfg, &subject, &body)
}

/// Raw TCP connect + banner read (like Test-NetConnection). Does not send mail.
// ponytail: wired up by the GUI's "Test connection" button (Task 9); no
// caller yet on either platform.
#[allow(dead_code)]
pub fn validate_smtp_connectivity(server: &str, port: u16, timeout_ms: u64) -> (bool, String) {
    use std::io::Read;
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let timeout = Duration::from_millis(timeout_ms);
    let addr = match format!("{server}:{port}").to_socket_addrs() {
        Ok(mut it) => match it.next() {
            Some(a) => a,
            None => return (false, format!("Cannot resolve {server}")),
        },
        Err(e) => return (false, format!("Cannot resolve {server}: {e}")),
    };
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(mut s) => {
            let _ = s.set_read_timeout(Some(timeout));
            let mut buf = [0u8; 1024];
            let banner = match s.read(&mut buf) {
                Ok(n) => String::from_utf8_lossy(&buf[..n]).trim().to_string(),
                Err(_) => String::new(),
            };
            (true, format!("Connected to {server}:{port}\r\nBanner: {banner}"))
        }
        Err(e) => (false, format!("Connection failed: {e}")),
    }
}

pub fn post_webhook(url: &str, payload: &WebhookPayload) {
    let result = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .post(url)
        .send_json(payload);
    match result {
        Ok(_) => logger::log("Webhook", "Webhook notification sent."),
        Err(e) => logger::log("Webhook", &format!("Webhook failed: {e}")),
    }
}

// ── Background queue: bounded, panic-isolated, never blocks the monitor ──

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct NotifyQueue {
    tx: std::sync::mpsc::SyncSender<Job>,
}

impl NotifyQueue {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(64);
        std::thread::spawn(move || {
            for job in rx {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            }
        });
        NotifyQueue { tx }
    }

    pub fn enqueue(&self, job: Job) {
        if self.tx.try_send(job).is_err() {
            logger::log("NotifyQ", "Notification queue full; dropping item.");
        }
    }

    pub fn enqueue_dump(&self, cfg: Config, dump_path: String) {
        if cfg.email_enabled {
            let c = cfg.clone();
            let p = dump_path.clone();
            self.enqueue(Box::new(move || {
                let (s, b) = dump_email(&c.target_name, &machine_name(), &p);
                match send_email(&c, &s, &b) {
                    Ok(()) => logger::log("NotifyQ", "Email: dump notification sent."),
                    Err(e) => logger::log("NotifyQ", &format!("Email failed: {e}")),
                }
            }));
        }
        if cfg.webhook_enabled {
            self.enqueue(Box::new(move || {
                let url = effective_webhook_url(&cfg);
                if !url.trim().is_empty() {
                    let payload = webhook_payload_dump(&cfg.target_name, &machine_name(), &dump_path);
                    post_webhook(&url, &payload);
                }
            }));
        }
    }

    pub fn enqueue_warning(&self, cfg: Config, subject: String, message: String) {
        if cfg.email_enabled {
            let c = cfg.clone();
            let (s2, m2) = (subject.clone(), message.clone());
            self.enqueue(Box::new(move || {
                if let Err(e) = send_email(&c, &s2, &m2) {
                    logger::log("NotifyQ", &format!("Warning email failed: {e}"));
                }
            }));
        }
        if cfg.webhook_enabled {
            self.enqueue(Box::new(move || {
                let url = effective_webhook_url(&cfg);
                if !url.trim().is_empty() {
                    post_webhook(&url, &webhook_payload_warning(&subject, &message));
                }
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_splitting() {
        assert_eq!(split_addresses(" a@x.com ; ;b@y.com;"), vec!["a@x.com", "b@y.com"]);
        assert!(split_addresses("").is_empty());
    }

    #[test]
    fn tls_mode_selection_matches_csharp() {
        assert_eq!(tls_mode(true, 465), TlsMode::Wrapper);
        assert_eq!(tls_mode(true, 587), TlsMode::Required);
        assert_eq!(tls_mode(false, 25), TlsMode::Opportunistic);
    }

    #[test]
    fn dump_email_format() {
        let (subject, body) = dump_email("MyApp", "SERVER01", r"C:\Dumps\MyApp_1.dmp");
        assert_eq!(subject, "[LogDump] Dump created for MyApp on SERVER01");
        assert!(body.contains("Target:     MyApp"));
        assert!(body.contains("Computer:   SERVER01"));
        assert!(body.contains(r"Dump File:  C:\Dumps\MyApp_1.dmp"));
        assert!(body.contains("Timestamp:  "));
    }

    #[test]
    fn webhook_payload_is_messagecard() {
        let p = webhook_payload_dump("MyApp", "SERVER01", r"C:\d\x.dmp");
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""@type":"MessageCard""#));
        assert!(json.contains(r#""themeColor":"FF0000""#));
        assert!(json.contains("Dump created for MyApp"));
        let w = webhook_payload_warning("subj", "msg");
        assert_eq!(w.theme_color, "FFAA00");
    }

    #[test]
    fn queue_executes_work_and_survives_panic() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let q = NotifyQueue::new();
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = n.clone();
        q.enqueue(Box::new(move || { n2.fetch_add(1, Ordering::SeqCst); }));
        q.enqueue(Box::new(|| panic!("notifier blew up")));
        let n3 = n.clone();
        q.enqueue(Box::new(move || { n3.fetch_add(1, Ordering::SeqCst); }));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(n.load(Ordering::SeqCst), 2, "work after a panicking job must still run");
    }
}

//! Native log-collection engine (spec: docs/superpowers/specs/
//! 2026-07-25-log-collector-design.md). Ports the three real workflows of
//! CCURE_LogCollector_GUI_v2.0.ps1 (Data Collection, Install Logs, System
//! Health) plus a LogDump support bundle. Collection shells only to
//! built-in Windows tools (robocopy, wevtutil, reg.exe, systeminfo,
//! powershell -Command, tar.exe) — no new crates, no shipped scripts.
//!
//! Layout mirrors the PS1: `<base>\YYYY-MM-DD\Run_HHMMSS\` containing
//! `Collection_Summary.txt`, `Run_Transcript.txt` and one zip per workflow.

pub mod discover;
#[cfg(windows)]
pub mod datacoll;
#[cfg(windows)]
pub mod installlogs;
#[cfg(windows)]
pub mod syshealth;
#[cfg(windows)]
pub mod pdm_bundle;

// Pure helpers (naming, redaction, filters) compile everywhere for tests;
// everything that spawns a process is #[cfg(windows)].
use std::path::{Path, PathBuf};

/// One run = one timestamped folder; every workflow appends to the shared
/// transcript and summary, then zips its own staging dir.
pub struct RunContext<'a> {
    pub run_dir: PathBuf,
    summary: Vec<String>,
    progress: Box<dyn FnMut(&str) + Send + 'a>,
}

/// `YYYY-MM-DD` / `Run_HHMMSS` names for a run started now.
pub fn run_folder_names(now: &chrono::DateTime<chrono::Local>) -> (String, String) {
    (now.format("%Y-%m-%d").to_string(), now.format("Run_%H%M%S").to_string())
}

impl<'a> RunContext<'a> {
    /// Creates `<base>\YYYY-MM-DD\Run_HHMMSS\`. `progress` receives every
    /// transcript line (GUI pipes it to the status label, CLI to stdout).
    pub fn start(base: &Path, progress: Box<dyn FnMut(&str) + Send + 'a>) -> std::io::Result<Self> {
        let (day, run) = run_folder_names(&chrono::Local::now());
        let run_dir = base.join(day).join(run);
        std::fs::create_dir_all(&run_dir)?;
        let mut ctx = RunContext { run_dir, summary: Vec::new(), progress };
        ctx.log("Run started.");
        Ok(ctx)
    }

    /// Transcript + progress. Never fails; transcript IO errors are ignored
    /// (collection must not die because logging did).
    pub fn log(&mut self, msg: &str) {
        (self.progress)(msg);
        let line = format!("[{}] {}\r\n", chrono::Local::now().format("%H:%M:%S"), msg);
        let path = self.run_dir.join("Run_Transcript.txt");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(line.as_bytes());
        }
    }

    /// One line in Collection_Summary.txt (what was collected / skipped).
    pub fn summarize(&mut self, line: &str) {
        self.summary.push(line.to_string());
    }

    /// Writes Collection_Summary.txt and returns the run folder.
    pub fn finish(mut self) -> PathBuf {
        self.log("Run finished.");
        let mut text = String::from("LogDump Collection Summary\r\n");
        text.push_str(&format!("Version: {}\r\n", env!("CARGO_PKG_VERSION")));
        text.push_str(&format!("Machine: {}\r\n", std::env::var("COMPUTERNAME").unwrap_or_default()));
        text.push_str(&format!("Time: {}\r\n\r\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
        for l in &self.summary {
            text.push_str(l);
            text.push_str("\r\n");
        }
        let _ = std::fs::write(self.run_dir.join("Collection_Summary.txt"), text);
        self.run_dir
    }
}

/// SMTP/webhook secrets never leave the machine inside a bundle: blank the
/// DPAPI blobs and username in a config JSON string. Pure so it's testable.
pub fn redact_config_json(json: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(mut v) => {
            if let Some(o) = v.as_object_mut() {
                for k in ["EncryptedPasswordBlob", "EncryptedWebhookUrlBlob", "SmtpUsername"] {
                    if o.contains_key(k) {
                        o.insert(k.into(), serde_json::Value::String("<redacted>".into()));
                    }
                }
            }
            serde_json::to_string_pretty(&v).unwrap_or_else(|_| json.to_string())
        }
        Err(_) => json.to_string(),
    }
}

/// wevtutil XPath for "events in the last `days` days" (empty = full export).
pub fn evtx_query(days: Option<u32>) -> Option<String> {
    days.map(|d| {
        let ms: u64 = u64::from(d) * 24 * 60 * 60 * 1000;
        format!("*[System[TimeCreated[timediff(@SystemTime) <= {ms}]]]")
    })
}

/// Port of the PS1's Convert-ToSafeName: invalid filename chars and
/// whitespace runs -> underscores.
pub fn safe_name(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_ws = false;
    for c in text.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || (c as u32) < 32;
        if c.is_whitespace() {
            in_ws = true;
            continue;
        }
        if in_ws {
            out.push('_');
            in_ws = false;
        }
        out.push(if invalid { '_' } else { c });
    }
    out.trim_matches('_').to_string()
}

// ---------------------------------------------------------------- windows --

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Spawn a console tool hidden and capture stdout+stderr. Never inherits a
/// console (GUI pump stalls otherwise — see CLAUDE.md).
#[cfg(windows)]
pub fn run_tool(exe: &str, args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("{exe}: {e}"))
}

/// Robocopy a folder (best-effort: /R:1 /W:1, no VSS). Exit codes < 8 are
/// success/partial-with-extras; >= 8 means real copy errors (logged, still
/// treated as partial success like the PS1).
#[cfg(windows)]
pub fn robocopy(ctx: &mut RunContext, src: &Path, dest: &Path) -> bool {
    if !src.exists() {
        ctx.log(&format!("WARN: missing path: {}", src.display()));
        return false;
    }
    let _ = std::fs::create_dir_all(dest);
    let s = src.to_string_lossy().to_string();
    let d = dest.to_string_lossy().to_string();
    match run_tool(
        "robocopy",
        &[&s, &d, "/E", "/Z", "/R:1", "/W:1", "/COPY:DAT", "/DCOPY:DAT", "/XJ", "/NFL", "/NDL", "/NP"],
    ) {
        Ok(out) => {
            let code = out.status.code().unwrap_or(16);
            if code >= 8 {
                ctx.log(&format!(
                    "WARN: robocopy exit {code} for {} (locked/in-use files skipped)",
                    src.display()
                ));
            } else {
                ctx.log(&format!("Copied {}", src.display()));
            }
            true
        }
        Err(e) => {
            ctx.log(&format!("WARN: robocopy failed: {e}"));
            false
        }
    }
}

/// Zip `staging` into `zip_path` and delete the staging dir. tar.exe (bsdtar,
/// Win10 1803+/Server 2019+) first; PowerShell Compress-Archive fallback for
/// older servers.
#[cfg(windows)]
pub fn zip_dir(ctx: &mut RunContext, staging: &Path, zip_path: &Path) -> bool {
    let staging_s = staging.to_string_lossy().to_string();
    let zip_s = zip_path.to_string_lossy().to_string();
    let ok = match run_tool("tar.exe", &["-a", "-c", "-f", &zip_s, "-C", &staging_s, "."]) {
        Ok(out) if out.status.success() && zip_path.exists() => true,
        _ => {
            ctx.log("tar.exe unavailable/failed - falling back to Compress-Archive");
            let cmd = format!(
                "Compress-Archive -Path '{}\\*' -DestinationPath '{}' -Force",
                staging_s.replace('\'', "''"),
                zip_s.replace('\'', "''")
            );
            matches!(run_powershell(&cmd), Ok(out) if out.status.success() && zip_path.exists())
        }
    };
    if ok {
        ctx.log(&format!("Created {}", zip_path.display()));
        let _ = std::fs::remove_dir_all(staging);
    } else {
        ctx.log(&format!(
            "WARN: could not zip {} - leaving folder unzipped",
            staging.display()
        ));
    }
    ok
}

/// Inline `powershell -Command` (PowerShell 5.1 is on every supported
/// server; -Command is exempt from execution policy concerns for scripts).
#[cfg(windows)]
pub fn run_powershell(command: &str) -> Result<std::process::Output, String> {
    run_tool(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", command],
    )
}

/// Export Application + System event logs as .evtx into `dir`.
/// `days: None` = full export, `Some(n)` = last n days.
#[cfg(windows)]
pub fn export_event_logs(ctx: &mut RunContext, dir: &Path, days: Option<u32>) {
    let _ = std::fs::create_dir_all(dir);
    let query = evtx_query(days);
    for log in ["Application", "System"] {
        let dest = dir.join(format!("{log}.evtx"));
        let dest_s = dest.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["epl", log, &dest_s, "/ow:true"];
        let q_arg;
        if let Some(q) = &query {
            q_arg = format!("/q:{q}");
            args.push(&q_arg);
        }
        match run_tool("wevtutil.exe", &args) {
            Ok(out) if out.status.success() => {
                ctx.log(&format!("Exported {log} event log"));
            }
            Ok(out) => ctx.log(&format!(
                "WARN: wevtutil {log} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            Err(e) => ctx.log(&format!("WARN: {e}")),
        }
    }
}

/// Write a tool's stdout to a file in the run (logs stderr as WARN).
#[cfg(windows)]
pub fn capture_to_file(ctx: &mut RunContext, exe: &str, args: &[&str], dest: &Path) {
    match run_tool(exe, args) {
        Ok(out) => {
            let _ = std::fs::write(dest, &out.stdout);
            if !out.status.success() {
                ctx.log(&format!(
                    "WARN: {exe} exit {:?}: {}",
                    out.status.code(),
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            } else {
                ctx.log(&format!("Wrote {}", dest.display()));
            }
        }
        Err(e) => ctx.log(&format!("WARN: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_folder_names_shape() {
        let t = chrono::Local::now();
        let (day, run) = run_folder_names(&t);
        assert_eq!(day.len(), 10);
        assert!(run.starts_with("Run_") && run.len() == 10);
    }

    #[test]
    fn redaction_blanks_secrets_and_survives_bad_json() {
        let json = r#"{"SmtpServer":"m","SmtpUsername":"u","EncryptedPasswordBlob":"AAA=","EncryptedWebhookUrlBlob":"BBB="}"#;
        let red = redact_config_json(json);
        assert!(!red.contains("AAA=") && !red.contains("BBB=") && !red.contains("\"u\""));
        assert!(red.contains("<redacted>") && red.contains("\"m\""));
        assert_eq!(redact_config_json("{not json"), "{not json");
    }

    #[test]
    fn evtx_query_seven_days_and_full() {
        assert_eq!(
            evtx_query(Some(7)).unwrap(),
            "*[System[TimeCreated[timediff(@SystemTime) <= 604800000]]]"
        );
        assert!(evtx_query(None).is_none());
    }

    #[test]
    fn safe_name_ports_convert_to_safe_name() {
        assert_eq!(safe_name("CCure Portal logs"), "CCure_Portal_logs");
        assert_eq!(safe_name("a<b>:c|d?*"), "a_b__c_d");
        assert_eq!(safe_name("  edges  "), "edges");
    }

    #[test]
    fn run_context_writes_transcript_and_summary() {
        let base = std::env::temp_dir().join("pdm_collect_test");
        let _ = std::fs::remove_dir_all(&base);
        let mut lines: Vec<String> = Vec::new();
        let run_dir = {
            let mut ctx =
                RunContext::start(&base, Box::new(|s: &str| lines.push(s.to_string()))).unwrap();
            ctx.log("hello");
            ctx.summarize("Data Collection: OK");
            ctx.finish()
        };
        assert!(run_dir.join("Run_Transcript.txt").exists());
        let sum = std::fs::read_to_string(run_dir.join("Collection_Summary.txt")).unwrap();
        assert!(sum.contains("Data Collection: OK"));
        assert!(lines.iter().any(|l| l == "hello"));
    }
}

//! LogDump support bundle — the monitor's own evidence: log files,
//! health.json heartbeat, redacted config, scheduled-task state, dump-folder
//! listing and the newest dumps (size-capped). Used as a Data Collection
//! checkbox and as the auto-collect-on-dump mini bundle.

use super::RunContext;
use std::path::{Path, PathBuf};

pub struct Options {
    pub log_dir: PathBuf,
    pub health_path: PathBuf,
    pub config_path: PathBuf,
    /// Already-sanitized scheduled task name ("" = skip task query).
    pub task_name: String,
    pub dump_dir: PathBuf,
    /// Total byte cap for copied dump files (newest first, always >= 1 file).
    pub max_dump_bytes: u64,
}

pub const DEFAULT_MAX_DUMP_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GB

/// Newest-first (path, len) dump listing.
fn dumps_newest_first(dump_dir: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut v: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dump_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x.eq_ignore_ascii_case("dmp")).unwrap_or(false) {
                if let Ok(md) = e.metadata() {
                    let mtime = md.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    v.push((p, md.len(), mtime));
                }
            }
        }
    }
    v.sort_by(|a, b| b.2.cmp(&a.2));
    v
}

/// Which of the newest-first dumps fit the byte cap (always the newest one,
/// even if it alone exceeds the cap). Pure for tests.
pub fn pick_dumps_within_cap(sizes: &[u64], cap: u64) -> usize {
    let mut total = 0u64;
    let mut n = 0usize;
    for &s in sizes {
        if n > 0 && total.saturating_add(s) > cap {
            break;
        }
        total = total.saturating_add(s);
        n += 1;
        if total > cap {
            break;
        }
    }
    n
}

pub fn run_into(ctx: &mut RunContext, opts: &Options, dest: &Path) {
    ctx.log("PDMBundle: collecting LogDump state...");
    let _ = std::fs::create_dir_all(dest);

    // App logs (procdump.log + rotations).
    if opts.log_dir.exists() {
        super::robocopy(ctx, &opts.log_dir, &dest.join("Logs"));
    } else {
        ctx.log("PDMBundle: no log directory yet");
    }

    // health.json heartbeat.
    if opts.health_path.exists() {
        let _ = std::fs::copy(&opts.health_path, dest.join("health.json"));
    }

    // Config with secrets redacted.
    match std::fs::read_to_string(&opts.config_path) {
        Ok(json) => {
            let _ = std::fs::write(dest.join("config.redacted.json"), super::redact_config_json(&json));
            ctx.log("PDMBundle: config captured (secrets redacted)");
        }
        Err(_) => ctx.log("PDMBundle: no config.json found"),
    }

    // Scheduled-task state.
    if !opts.task_name.is_empty() {
        super::capture_to_file(
            ctx,
            "schtasks.exe",
            &["/query", "/v", "/fo", "LIST", "/tn", &opts.task_name],
            &dest.join("ScheduledTask.txt"),
        );
    }

    // Dump folder listing + newest dumps within the cap.
    let dumps = dumps_newest_first(&opts.dump_dir);
    let mut listing = String::from("Name\tSizeBytes\tModifiedUTC\r\n");
    for (p, len, mtime) in &dumps {
        let dt: chrono::DateTime<chrono::Utc> = (*mtime).into();
        listing.push_str(&format!(
            "{}\t{}\t{}\r\n",
            p.file_name().unwrap_or_default().to_string_lossy(),
            len,
            dt.format("%Y-%m-%d %H:%M:%S")
        ));
    }
    let _ = std::fs::write(dest.join("DumpFolderListing.txt"), listing);

    let sizes: Vec<u64> = dumps.iter().map(|d| d.1).collect();
    let n = pick_dumps_within_cap(&sizes, opts.max_dump_bytes);
    if n > 0 {
        let dump_dest = dest.join("Dumps");
        let _ = std::fs::create_dir_all(&dump_dest);
        for (p, _, _) in dumps.iter().take(n) {
            match std::fs::copy(p, dump_dest.join(p.file_name().unwrap_or_default())) {
                Ok(_) => ctx.log(&format!("PDMBundle: copied dump {}", p.display())),
                Err(e) => ctx.log(&format!("WARN: dump copy failed ({}): {e}", p.display())),
            }
        }
        if dumps.len() > n {
            ctx.log(&format!(
                "PDMBundle: {} older dump(s) listed but not copied (size cap)",
                dumps.len() - n
            ));
        }
    }
    ctx.summarize(&format!(
        "PDM bundle: logs, health, config (redacted), task state, {n}/{} dumps",
        dumps.len()
    ));
}

/// Auto-collect-on-dump: rate-limited mini bundle (PDM state + last-24h
/// event logs) zipped under `<dump_dir>\SupportBundles`. Returns the run
/// folder, or Err with the human-readable skip reason.
#[cfg(windows)]
pub fn auto_bundle(opts: &Options) -> Result<PathBuf, String> {
    let base = opts.dump_dir.join("SupportBundles");
    std::fs::create_dir_all(&base).map_err(|e| format!("create SupportBundles: {e}"))?;

    // Rate limit: one auto bundle per 60 minutes (crash-loop protection).
    // ponytail: fixed 60-min limit; make configurable only if a deployment needs it.
    let marker = base.join(".last_auto_collect");
    if let Ok(md) = std::fs::metadata(&marker) {
        if let Ok(mtime) = md.modified() {
            if mtime.elapsed().map(|e| e.as_secs() < 3600).unwrap_or(false) {
                return Err("rate-limited (last auto-collect < 60 min ago)".into());
            }
        }
    }

    let mut ctx = RunContext::start(&base, Box::new(|_s: &str| {}))
        .map_err(|e| format!("start run: {e}"))?;
    let staging = ctx.run_dir.join("AutoCollect");
    run_into(&mut ctx, opts, &staging);
    super::export_event_logs(&mut ctx, &staging.join("EventLogs"), Some(1));
    let zip = ctx.run_dir.join("AutoCollect.zip");
    super::zip_dir(&mut ctx, &staging, &zip);
    ctx.summarize("Auto-collect: AutoCollect.zip");
    let run_dir = ctx.finish();
    let _ = std::fs::write(&marker, chrono::Local::now().to_rfc3339());
    Ok(run_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_cap_always_takes_newest_then_fills() {
        // newest first: 3GB single dump still copied
        assert_eq!(pick_dumps_within_cap(&[3_000_000_000], 2_000_000_000), 1);
        // fills until cap
        assert_eq!(pick_dumps_within_cap(&[500, 500, 500, 500], 1200), 2);
        assert_eq!(pick_dumps_within_cap(&[], 100), 0);
        // exact fit
        assert_eq!(pick_dumps_within_cap(&[600, 600], 1200), 2);
    }
}

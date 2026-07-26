//! Install Logs workflow — port of the PS1's Invoke-InstallLogsCollector.

use super::{discover, RunContext};
use std::path::PathBuf;

pub struct Options {
    /// Explicit InstallHistory.xml path; None or `auto` -> discovery.
    pub history_path: Option<PathBuf>,
    pub auto_discover: bool,
    pub include_installer_temp: bool,
    pub include_install_cache: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            history_path: None,
            auto_discover: true,
            include_installer_temp: true,
            include_install_cache: true,
        }
    }
}

pub fn run(ctx: &mut RunContext, opts: &Options) {
    ctx.log("InstallLogs: starting extraction...");
    let program_data =
        PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into()));

    let ih_path = if opts.auto_discover || opts.history_path.is_none() {
        match discover::find_install_history(&program_data) {
            Some(p) => {
                ctx.log(&format!("InstallLogs: auto-selected {}", p.display()));
                Some(p)
            }
            None => opts.history_path.clone(),
        }
    } else {
        opts.history_path.clone()
    };

    let Some(ih_path) = ih_path.filter(|p| p.exists()) else {
        ctx.log("InstallLogs: ERROR - InstallHistory.xml not found");
        ctx.summarize("Install Logs: FAILED (InstallHistory.xml not found)");
        return;
    };

    let staging = ctx.run_dir.join("InstallLogs");
    let _ = std::fs::create_dir_all(&staging);

    match std::fs::copy(&ih_path, staging.join("InstallHistory.xml")) {
        Ok(_) => ctx.log("InstallLogs: copied InstallHistory.xml"),
        Err(e) => ctx.log(&format!("WARN: InstallHistory.xml copy failed: {e}")),
    }

    if opts.include_installer_temp {
        if let Some(parent) = ih_path.parent() {
            super::robocopy(ctx, parent, &staging.join("InstallerTemp"));
        }
    }

    if opts.include_install_cache {
        let mut found = false;
        for cache in discover::install_cache_candidates(&program_data) {
            if cache.exists() {
                found = super::robocopy(ctx, &cache, &staging.join("InstallCache"));
                break; // PS1 copies the first existing (JCI preferred)
            }
        }
        if !found {
            ctx.log("InstallLogs: WARN - InstallCache not found under ProgramData");
        }
    }

    let zip = ctx.run_dir.join("InstallLogs.zip");
    super::zip_dir(ctx, &staging, &zip);
    ctx.summarize("Install Logs: InstallLogs.zip");
    ctx.log("InstallLogs: done.");
}

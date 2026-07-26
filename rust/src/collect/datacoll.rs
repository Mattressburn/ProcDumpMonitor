//! Data Collection workflow — app/web log components + optional extras.
//! Port of the PS1's "Data Collection" tab (its Start-Collection path).

use super::{discover, RunContext};
use std::path::{Path, PathBuf};

pub struct Options {
    /// (display name, candidate paths) for each CHECKED log component.
    pub components: Vec<(String, Vec<PathBuf>)>,
    pub system_info: bool,
    pub installed_apps: bool,
    pub installed_updates: bool,
    pub event_logs: bool,
    /// false = last 7 days (new option), true = full export (PS1 behavior).
    pub event_logs_full: bool,
    pub install_history: bool,
    pub bulk_updates: bool,
    pub swh_settings: bool,
    /// New: ProcDumpMonitor's own logs/dumps/config/task state.
    pub pdm_bundle: Option<super::pdm_bundle::Options>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            components: Vec::new(),
            system_info: true,
            installed_apps: true,
            installed_updates: true,
            event_logs: true,
            event_logs_full: false,
            install_history: true,
            bulk_updates: false,
            swh_settings: true,
            pdm_bundle: None,
        }
    }
}

pub fn run(ctx: &mut RunContext, opts: &Options) {
    ctx.log("DataCollection: starting...");
    let staging = ctx.run_dir.join("DataCollection");
    let _ = std::fs::create_dir_all(&staging);

    // ---- Log components (install-dir based) ----
    let mut copied = 0usize;
    for (name, paths) in &opts.components {
        let mut any = false;
        for p in paths {
            if p.exists() {
                let dest = staging.join("Logs").join(super::safe_name(name));
                any |= super::robocopy(ctx, p, &dest);
            }
        }
        if any {
            copied += 1;
        } else {
            ctx.log(&format!("DataCollection: no existing path for component '{name}'"));
        }
    }
    ctx.summarize(&format!(
        "Data Collection: {copied}/{} log components captured",
        opts.components.len()
    ));

    // ---- Extras ----
    if opts.system_info {
        super::capture_to_file(ctx, "systeminfo.exe", &[], &staging.join("SystemInfo.txt"));
    }
    if opts.installed_apps {
        let cmd = r#"$k='HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*';Get-ItemProperty $k -ErrorAction SilentlyContinue|Where-Object DisplayName|Sort-Object DisplayName|Select-Object DisplayName,DisplayVersion,Publisher,InstallDate|ConvertTo-Csv -NoTypeInformation"#;
        ps_to_file(ctx, cmd, &staging.join("InstalledApplications.csv"));
    }
    if opts.installed_updates {
        let cmd = "Get-HotFix -ErrorAction SilentlyContinue|Sort-Object InstalledOn -Descending -ErrorAction SilentlyContinue|Select-Object HotFixID,Description,InstalledOn,InstalledBy|ConvertTo-Csv -NoTypeInformation";
        ps_to_file(ctx, cmd, &staging.join("InstalledUpdates.csv"));
    }
    if opts.event_logs {
        super::export_event_logs(
            ctx,
            &staging.join("EventLogs"),
            if opts.event_logs_full { None } else { Some(7) },
        );
        ctx.summarize(&format!(
            "Event logs: Application + System ({})",
            if opts.event_logs_full { "full" } else { "last 7 days" }
        ));
    }

    let program_data =
        PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into()));

    if opts.install_history {
        match discover::find_install_history(&program_data) {
            Some(p) => {
                let dest = staging.join("InstallerTemp");
                let _ = std::fs::create_dir_all(&dest);
                match std::fs::copy(&p, dest.join("InstallHistory.xml")) {
                    Ok(_) => ctx.log(&format!("DataCollection: copied {}", p.display())),
                    Err(e) => ctx.log(&format!("WARN: InstallHistory copy failed: {e}")),
                }
            }
            None => ctx.log("DataCollection: InstallHistory.xml not found"),
        }
    }

    if opts.bulk_updates || opts.swh_settings || !opts.components.is_empty() {
        // Discovery shared by bulk updates + used for the summary line.
        let loc = install_location_logged(ctx);
        if opts.bulk_updates {
            let exists = |p: &Path| p.exists();
            let (jci, tyco) = discover::vendor_roots(loc.as_deref(), &exists);
            let mut found = false;
            for dir in discover::bulk_update_candidates(loc.as_deref(), &jci, &tyco) {
                if dir.exists() {
                    found |= super::robocopy(ctx, &dir, &staging.join("BulkUpdates"));
                }
            }
            if !found {
                ctx.log("DataCollection: no bulk-update directories found");
            }
        }
    }

    if opts.swh_settings {
        let configs = discover::find_dashboard_configs(&program_data, 5);
        if configs.is_empty() {
            ctx.log("DataCollection: no Dashboard.exe.config found under ProgramData");
        } else {
            let dest = staging.join("SWHSystemSettings");
            let _ = std::fs::create_dir_all(&dest);
            for (i, p) in configs.iter().enumerate() {
                let name = format!("Dashboard.exe.config.{i}");
                let _ = std::fs::copy(p, dest.join(name));
            }
            ctx.log(&format!(
                "DataCollection: copied {} Dashboard.exe.config file(s) (SWHSystem settings)",
                configs.len()
            ));
        }
    }

    if let Some(pdm) = &opts.pdm_bundle {
        super::pdm_bundle::run_into(ctx, pdm, &staging.join("ProcDumpMonitor"));
    }

    let zip = ctx.run_dir.join("DataCollection.zip");
    super::zip_dir(ctx, &staging, &zip);
    ctx.summarize("Data Collection: DataCollection.zip");
    ctx.log("DataCollection: done.");
}

/// InstallLocation from the registry, logged once.
fn install_location_logged(ctx: &mut RunContext) -> Option<String> {
    let loc = discover::install_location();
    match &loc {
        Some(l) => ctx.log(&format!("DataCollection: CCURE InstallLocation: {l}")),
        None => ctx.log("DataCollection: CCURE InstallLocation not in registry - using defaults"),
    }
    loc
}

/// Run an inline PowerShell pipeline and write its stdout to `dest`.
fn ps_to_file(ctx: &mut RunContext, command: &str, dest: &Path) {
    match super::run_powershell(command) {
        Ok(out) if out.status.success() => {
            let _ = std::fs::write(dest, &out.stdout);
            ctx.log(&format!("Wrote {}", dest.display()));
        }
        Ok(out) => ctx.log(&format!(
            "WARN: powershell failed for {}: {}",
            dest.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => ctx.log(&format!("WARN: {e}")),
    }
}

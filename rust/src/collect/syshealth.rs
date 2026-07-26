//! System Health workflow — uptime + process + service snapshots.
//! Port of Invoke-SystemHealthCollector. The rich snapshots come from one
//! inline `powershell -Command` each (PowerShell 5.1 ships on every
//! supported server; -Command needs no execution-policy changes), filtered
//! by the same comma-separated substring patterns as the PS1.

use super::RunContext;

pub struct Options {
    pub uptime: bool,
    pub processes: bool,
    pub services: bool,
    pub proc_patterns: String,
    pub svc_patterns: String,
}

/// The PS1 v2.0 default match patterns (shared by both boxes).
pub const DEFAULT_PATTERNS: &str = "ccure,crossfire,tyco,jci,victor,swh,security,intelligence,dashboard,mq,acvs,dmp,dsc,bosch,istar,kone,galaxy,itv2,sql,stunnel,AD,db,search";

impl Default for Options {
    fn default() -> Self {
        Options {
            uptime: true,
            processes: true,
            services: true,
            proc_patterns: DEFAULT_PATTERNS.into(),
            svc_patterns: DEFAULT_PATTERNS.into(),
        }
    }
}

/// PowerShell `-like "*p*"` filter clause over `$_.{props}` from a
/// comma-separated pattern list; empty list -> `$true` (match all).
pub fn ps_filter(patterns: &str, props: &[&str]) -> String {
    let pats: Vec<String> = patterns
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.replace('\'', "''").replace('"', "`\""))
        .collect();
    if pats.is_empty() {
        return "$true".into();
    }
    let clauses: Vec<String> = pats
        .iter()
        .flat_map(|p| props.iter().map(move |prop| format!("$_.{prop} -like \"*{p}*\"")))
        .collect();
    clauses.join(" -or ")
}

pub fn run(ctx: &mut RunContext, opts: &Options) {
    ctx.log("SystemHealth: starting...");
    let staging = ctx.run_dir.join("SystemHealth");
    let _ = std::fs::create_dir_all(&staging);

    if opts.uptime {
        let cmd = r#"$os=Get-CimInstance Win32_OperatingSystem;$b=$os.LastBootUpTime;$u=(Get-Date)-$b;[pscustomobject]@{ComputerName=$env:COMPUTERNAME;LastBootUpTime=$b.ToString('o');Uptime=('{0:dd\.hh\:mm\:ss}' -f $u);UptimeDays=[math]::Round($u.TotalDays,2)}|ConvertTo-Json"#;
        run_ps_capture(ctx, cmd, &staging, "Uptime.json");
    }

    if opts.processes {
        ctx.log("SystemHealth: collecting processes...");
        let filter = ps_filter(&opts.proc_patterns, &["Name"]);
        let cmd = format!(
            r#"$p=Get-Process -ErrorAction SilentlyContinue|Where-Object {{ {filter} }}|ForEach-Object {{ $exe=$null;try{{$exe=$_.Path}}catch{{}};$vi=$null;if($exe -and (Test-Path -LiteralPath $exe)){{try{{$vi=[System.Diagnostics.FileVersionInfo]::GetVersionInfo($exe)}}catch{{}}}};[pscustomobject]@{{Name=$_.Name;Id=$_.Id;SessionId=$_.SessionId;CPUSeconds=$_.CPU;Handles=$_.Handles;Threads=$_.Threads.Count;WorkingSetMB=[math]::Round($_.WorkingSet64/1MB,2);PrivateMB=[math]::Round($_.PagedMemorySize64/1MB,2);VirtualMB=[math]::Round($_.VirtualMemorySize64/1MB,2);PeakWorkingSetMB=[math]::Round($_.PeakWorkingSet64/1MB,2);StartTime=$(try{{$_.StartTime.ToString('o')}}catch{{$null}});Path=$exe;Responding=$_.Responding;FileCompany=$vi.CompanyName;FileProduct=$vi.ProductName;FileVersion=$vi.FileVersion;FileDescription=$vi.FileDescription}} }}|Sort-Object CPUSeconds -Descending;$p|ConvertTo-Json -Depth 4|Out-File -LiteralPath '{{OUT}}\Processes.json' -Encoding UTF8;$p|Export-Csv -Path '{{OUT}}\Processes.csv' -NoTypeInformation -Encoding UTF8;($p|Format-Table -Wrap|Out-String)|Out-File -LiteralPath '{{OUT}}\Processes.txt' -Encoding UTF8;@($p).Count"#
        );
        run_ps_snapshot(ctx, &cmd, &staging, "process");
    }

    if opts.services {
        ctx.log("SystemHealth: collecting services...");
        let filter = ps_filter(&opts.svc_patterns, &["Name", "DisplayName"]);
        let cmd = format!(
            r#"$m=@{{}};Get-Service -ErrorAction SilentlyContinue|ForEach-Object {{ $m[$_.Name]=@{{Status=[string]$_.Status;DependsOn=(@($_.ServicesDependedOn|ForEach-Object Name) -join ';');Dependent=(@($_.DependentServices|ForEach-Object Name) -join ';');CanStop=$_.CanStop}} }};$s=Get-CimInstance Win32_Service -ErrorAction Stop|Where-Object {{ {filter} }}|ForEach-Object {{ $c=$m[$_.Name];[pscustomobject]@{{Name=$_.Name;DisplayName=$_.DisplayName;State=$_.State;Status=$c.Status;StartMode=$_.StartMode;StartName=$_.StartName;ProcessId=$_.ProcessId;ServiceType=$_.ServiceType;PathName=$_.PathName;ExitCode=$_.ExitCode;Win32ExitCode=$_.Win32ExitCode;ErrorControl=$_.ErrorControl;DependsOnServices=$c.DependsOn;DependentServices=$c.Dependent;CanStop=$c.CanStop;Description=$_.Description}} }}|Sort-Object Name;$s|ConvertTo-Json -Depth 4|Out-File -LiteralPath '{{OUT}}\Services.json' -Encoding UTF8;$s|Export-Csv -Path '{{OUT}}\Services.csv' -NoTypeInformation -Encoding UTF8;($s|Format-Table -AutoSize|Out-String)|Out-File -LiteralPath '{{OUT}}\Services.txt' -Encoding UTF8;@($s).Count"#
        );
        run_ps_snapshot(ctx, &cmd, &staging, "service");
    }

    let zip = ctx.run_dir.join("SystemHealth.zip");
    super::zip_dir(ctx, &staging, &zip);
    ctx.summarize("System Health: SystemHealth.zip");
    ctx.log("SystemHealth: done.");
}

/// Run a snapshot pipeline after substituting {OUT} with the staging dir.
#[cfg(windows)]
fn run_ps_snapshot(ctx: &mut RunContext, cmd_template: &str, staging: &std::path::Path, what: &str) {
    let out_dir = staging.to_string_lossy().replace('\'', "''");
    let cmd = cmd_template.replace("{OUT}", &out_dir);
    match super::run_powershell(&cmd) {
        Ok(out) if out.status.success() => {
            let count = String::from_utf8_lossy(&out.stdout).trim().to_string();
            ctx.log(&format!("SystemHealth: {what} snapshot saved (count={count})"));
            ctx.summarize(&format!("System Health: {count} {what}(es) matched"));
        }
        Ok(out) => ctx.log(&format!(
            "WARN: {what} snapshot failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => ctx.log(&format!("WARN: {e}")),
    }
}

#[cfg(windows)]
fn run_ps_capture(ctx: &mut RunContext, cmd: &str, staging: &std::path::Path, file: &str) {
    match super::run_powershell(cmd) {
        Ok(out) if out.status.success() => {
            let _ = std::fs::write(staging.join(file), &out.stdout);
            ctx.log(&format!("SystemHealth: {file} collected"));
        }
        Ok(out) => ctx.log(&format!(
            "WARN: {file} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => ctx.log(&format!("WARN: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_filter_builds_like_clauses() {
        let f = ps_filter("ccure, sql", &["Name", "DisplayName"]);
        assert!(f.contains(r#"$_.Name -like "*ccure*""#));
        assert!(f.contains(r#"$_.DisplayName -like "*sql*""#));
        assert_eq!(f.matches(" -or ").count(), 3);
    }

    #[test]
    fn ps_filter_empty_matches_all_and_escapes_quotes() {
        assert_eq!(ps_filter("", &["Name"]), "$true");
        assert_eq!(ps_filter(" , ", &["Name"]), "$true");
        let f = ps_filter("o'brien", &["Name"]);
        assert!(f.contains("o''brien"));
    }
}

#![allow(dead_code)] // consumed from Task 8 onward

#[cfg(windows)]
use crate::config::Config;

pub fn sanitize_task_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect()
}

pub fn auto_task_name(target: &str) -> String {
    format!("ProcDump Monitor {target}")
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;").replace('\'', "&apos;")
}

pub fn to_utf16le_bom(s: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// Task Scheduler XML, format proven by live spike on win11-lab 2026-07-21.
/// SYSTEM principal: UserId only, NO LogonType (schtasks rejects ServiceAccount).
pub fn task_xml(target_name: &str, exe: &str, config_path: &str, workdir: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>ProcDump Monitor - watches for {target} and captures crash dumps.</Description>
  </RegistrationInfo>
  <Triggers>
    <BootTrigger>
      <Enabled>true</Enabled>
    </BootTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-18</UserId>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>true</Enabled>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>--monitor --config &quot;{config}&quot;</Arguments>
      <WorkingDirectory>{workdir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
        target = xml_escape(target_name),
        exe = xml_escape(exe),
        config = xml_escape(config_path),
        workdir = xml_escape(workdir),
    )
}

#[derive(Debug, Default, serde::Serialize)]
pub struct TaskStatus {
    #[serde(rename = "TaskName")] pub task_name: String,
    #[serde(rename = "MachineName")] pub machine_name: String,
    #[serde(rename = "Exists")] pub exists: bool,
    #[serde(rename = "State")] pub state: String,
    #[serde(rename = "LastRunTime")] pub last_run_time: String,
    #[serde(rename = "LastRunResult")] pub last_run_result: String,
    #[serde(rename = "NextRunTime")] pub next_run_time: String,
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::{logger, paths};
    use std::process::Command;

    fn schtasks(args: &[&str]) -> Result<String, String> {
        let out = Command::new("schtasks.exe")
            .args(args)
            .output()
            .map_err(|e| format!("cannot run schtasks: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if out.status.success() {
            Ok(stdout)
        } else {
            Err(if stderr.trim().is_empty() { stdout } else { stderr })
        }
    }

    /// Create-or-update. Returns Ok(true) if a task with this name already existed.
    pub fn install(cfg: &Config) -> Result<bool, String> {
        let name = sanitize_task_name(&cfg.task_name);
        if name.trim().is_empty() {
            return Err("Task name is empty after sanitisation.".into());
        }
        let exe = paths::exe_path().display().to_string();
        let config_path = paths::config_path().display().to_string();
        let workdir = paths::install_dir().display().to_string();
        let existed = exists(&name);

        let xml = task_xml(&cfg.target_name, &exe, &config_path, &workdir);
        let xml_file = std::env::temp_dir().join("pdm_task.xml");
        std::fs::write(&xml_file, to_utf16le_bom(&xml))
            .map_err(|e| format!("cannot write task xml: {e}"))?;

        let res = schtasks(&["/Create", "/TN", &name, "/XML",
                             &xml_file.display().to_string(), "/F"]);
        let _ = std::fs::remove_file(&xml_file);
        res?;
        logger::log("TaskSvc", &format!("Task '{name}' registered (existed={existed})."));
        Ok(existed)
    }

    pub fn uninstall(task_name: &str) -> Result<(), String> {
        schtasks(&["/Delete", "/TN", task_name, "/F"]).map(|_| ())
    }

    pub fn start(task_name: &str) -> Result<(), String> {
        schtasks(&["/Run", "/TN", task_name]).map(|_| ())
    }

    pub fn stop(task_name: &str) -> Result<(), String> {
        schtasks(&["/End", "/TN", task_name]).map(|_| ())
    }

    pub fn exists(task_name: &str) -> bool {
        schtasks(&["/Query", "/TN", task_name]).is_ok()
    }

    /// Parses `/Query /V /FO CSV` positionally (headers are localized; the
    /// column ORDER is stable): 0=HostName 1=TaskName 2=NextRunTime 3=Status
    /// 5=LastRunTime 6=LastResult.
    pub fn query_status(task_name: &str) -> TaskStatus {
        let mut st = TaskStatus {
            task_name: task_name.into(),
            machine_name: std::env::var("COMPUTERNAME").unwrap_or_default(),
            state: "Not installed".into(),
            ..Default::default()
        };
        let Ok(csv) = schtasks(&["/Query", "/TN", task_name, "/V", "/FO", "CSV"]) else {
            return st;
        };
        let Some(data_line) = csv.lines().nth(1) else { return st };
        let cols = parse_csv_line(data_line);
        if cols.len() > 6 {
            st.exists = true;
            st.next_run_time = cols[2].clone();
            st.state = cols[3].clone();
            st.last_run_time = cols[5].clone();
            st.last_run_result = cols[6].clone();
        }
        st
    }
}

#[cfg(windows)]
pub use win::*;

/// Minimal CSV field splitter for schtasks output (quoted fields, comma sep).
pub fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => { cur.push('"'); chars.next(); }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => { fields.push(std::mem::take(&mut cur)); }
            _ => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_reserved_chars() {
        assert_eq!(sanitize_task_name(r#"a\b/c:d*e?f"g<h>i|j"#), "abcdefghij");
        assert_eq!(auto_task_name("MyApp"), "ProcDump Monitor MyApp");
    }

    #[test]
    fn xml_matches_proven_spike_structure() {
        let xml = task_xml("MyApp", r"C:\Tools\ProcDumpMonitor.exe",
                           r"C:\Tools\config.json", r"C:\Tools");
        // Landmines proven on the VM 2026-07-21:
        assert!(!xml.contains("<LogonType>"), "LogonType must be omitted for SYSTEM");
        assert!(xml.contains("<UserId>S-1-5-18</UserId>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<BootTrigger>"));
        assert!(xml.contains("<Interval>PT1M</Interval>"));
        assert!(xml.contains("<Count>999</Count>"));
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<StartWhenAvailable>true</StartWhenAvailable>"));
        assert!(xml.contains(r"<Command>C:\Tools\ProcDumpMonitor.exe</Command>"));
        assert!(xml.contains(r#"<Arguments>--monitor --config &quot;C:\Tools\config.json&quot;</Arguments>"#));
        assert!(xml.contains(r"<WorkingDirectory>C:\Tools</WorkingDirectory>"));
        assert!(xml.contains("watches for MyApp"));
    }

    #[test]
    fn xml_escapes_special_chars() {
        let xml = task_xml("A&B", r"C:\Tools & Co\p.exe", r"C:\x.json", r"C:\y");
        assert!(xml.contains("A&amp;B"));
        assert!(xml.contains(r"C:\Tools &amp; Co\p.exe"));
    }

    #[test]
    fn utf16le_bom_encoding() {
        let bytes = to_utf16le_bom("<a/>");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE], "BOM required");
        assert_eq!(bytes.len(), 2 + 2 * 4);
        assert_eq!(&bytes[2..4], &[b'<', 0x00]);
    }

    #[test]
    fn csv_line_parsing_handles_quotes() {
        let cols = parse_csv_line(r#""HOST","\PDM Task","N/A","Ready","x","07/21/2026 4:00:00 PM","0""#);
        assert_eq!(cols[1], r"\PDM Task");
        assert_eq!(cols[3], "Ready");
        assert_eq!(cols[6], "0");
    }
}

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct HealthStatus {
    pub monitor_pid: u32,
    pub proc_dump_pid: u32,
    pub last_cycle_utc: String,
    pub last_proc_dump_exit_code: i32,
    pub last_dump_file_name: String,
    pub total_dump_count: i32,
    pub last_error: String,
    pub next_retry_utc: String,
    pub last_notified_dump_file: String,
    pub last_notified_utc: String,
    pub disk_space_low: bool,
    #[serde(rename = "FreeDiskMB")]
    pub free_disk_mb: i64,
    pub version: String,
}

/// Atomic write (tmp + rename) so monitors never read a torn file. Never panics.
pub fn write(path: &Path, status: &HealthStatus) {
    let Ok(json) = serde_json::to_string_pretty(status) else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

pub fn load(path: &Path) -> HealthStatus {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_csharp_names() {
        let dir = std::env::temp_dir().join("pdm_health");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("health.json");
        let mut h = HealthStatus::default();
        h.total_dump_count = 7;
        h.last_notified_dump_file = "x.dmp".into();
        write(&p, &h);
        let json = std::fs::read_to_string(&p).unwrap();
        for k in ["\"MonitorPid\"", "\"ProcDumpPid\"", "\"TotalDumpCount\"",
                  "\"LastNotifiedDumpFile\"", "\"FreeDiskMB\"", "\"DiskSpaceLow\""] {
            assert!(json.contains(k), "missing {k}");
        }
        let loaded = load(&p);
        assert_eq!(loaded.total_dump_count, 7);
        // missing/corrupt -> default
        assert_eq!(load(&dir.join("nope.json")).total_dump_count, 0);
    }
}

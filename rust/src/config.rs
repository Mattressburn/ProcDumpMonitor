#![allow(dead_code)] // consumed from Task 3 onward

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CURRENT_VERSION: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetType {
    #[default]
    Process,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Config {
    pub config_version: i32,
    pub target_name: String,
    pub target_type: TargetType,
    pub proc_dump_path: String,
    pub dump_directory: String,
    pub dump_type: String,          // Full | MiniPlus | Mini | ThreadDump
    pub dump_on_exception: bool,    // -e
    pub dump_on_terminate: bool,    // -t
    pub use_clone: bool,            // -r
    pub max_dumps: i32,             // -n
    pub restart_delay_seconds: i32,
    pub scenario: String,           // "" = Custom
    pub avoid_outage: bool,         // -a
    pub overwrite_existing: bool,   // -o
    pub wait_for_process: bool,     // -w
    pub cpu_per_unit: bool,         // -u
    pub cpu_duration_seconds: i32,  // -s
    pub cpu_threshold: i32,         // -c
    pub cpu_low_threshold: i32,     // -cl
    #[serde(rename = "MemoryCommitMB")]
    pub memory_commit_mb: i32,      // -m
    pub hang_window_seconds: i32,   // >0 -> -h
    pub performance_counter: String,     // -p
    pub perf_counter_threshold: String,  // -pl
    pub exception_filter_include: String, // -f
    pub exception_filter_exclude: String, // -fx
    pub wer_integration: bool,      // -wer
    pub avoid_terminate_timeout: i32, // -at
    #[serde(rename = "MinFreeDiskMB")]
    pub min_free_disk_mb: i64,
    pub dump_stability_timeout_seconds: i32,
    pub dump_stability_poll_seconds: i32,
    #[serde(rename = "MaxLogSizeMB")]
    pub max_log_size_mb: i32,
    pub max_log_files: i32,
    pub dump_retention_days: i32,
    #[serde(rename = "DumpRetentionMaxGB")]
    pub dump_retention_max_gb: f64,
    pub task_name: String,
    pub email_enabled: bool,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub use_ssl: bool,
    pub from_address: String,
    pub to_address: String,   // semicolon-delimited
    pub cc_address: String,   // semicolon-delimited
    pub smtp_username: String,
    pub encrypted_password_blob: String,     // base64 DPAPI blob
    pub webhook_enabled: bool,
    pub webhook_url: String,                 // plaintext (encrypted on save)
    pub encrypted_webhook_url_blob: String,  // base64 DPAPI blob
}

impl Default for Config {
    fn default() -> Self {
        Config {
            config_version: CURRENT_VERSION,
            target_name: String::new(),
            target_type: TargetType::Process,
            proc_dump_path: String::new(),
            dump_directory: String::new(),
            dump_type: "Full".into(),
            dump_on_exception: true,
            dump_on_terminate: true,
            use_clone: true,
            max_dumps: 1,
            restart_delay_seconds: 5,
            scenario: "Crash capture".into(),
            avoid_outage: false,
            overwrite_existing: false,
            wait_for_process: true,
            cpu_per_unit: false,
            cpu_duration_seconds: 0,
            cpu_threshold: 0,
            cpu_low_threshold: 0,
            memory_commit_mb: 0,
            hang_window_seconds: 0,
            performance_counter: String::new(),
            perf_counter_threshold: String::new(),
            exception_filter_include: String::new(),
            exception_filter_exclude: String::new(),
            wer_integration: false,
            avoid_terminate_timeout: 0,
            min_free_disk_mb: 5120,
            dump_stability_timeout_seconds: 30,
            dump_stability_poll_seconds: 2,
            max_log_size_mb: 10,
            max_log_files: 5,
            dump_retention_days: 0,
            dump_retention_max_gb: 0.0,
            task_name: "ProcDump Monitor".into(),
            email_enabled: false,
            smtp_server: String::new(),
            smtp_port: 25,
            use_ssl: false,
            from_address: String::new(),
            to_address: String::new(),
            cc_address: String::new(),
            smtp_username: String::new(),
            encrypted_password_blob: String::new(),
            webhook_enabled: false,
            webhook_url: String::new(),
            encrypted_webhook_url_blob: String::new(),
        }
    }
}

impl Config {
    /// Missing or unparseable file -> defaults (matches C# behavior).
    pub fn load(path: &Path) -> Config {
        match std::fs::read_to_string(path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        self.config_version = CURRENT_VERSION;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_csharp() {
        let c = Config::default();
        assert_eq!(c.config_version, 3);
        assert_eq!(c.dump_type, "Full");
        assert!(c.dump_on_exception && c.dump_on_terminate && c.use_clone);
        assert_eq!(c.max_dumps, 1);
        assert_eq!(c.restart_delay_seconds, 5);
        assert_eq!(c.scenario, "Crash capture");
        assert!(c.wait_for_process);
        assert_eq!(c.min_free_disk_mb, 5120);
        assert_eq!(c.dump_stability_timeout_seconds, 30);
        assert_eq!(c.dump_stability_poll_seconds, 2);
        assert_eq!(c.max_log_size_mb, 10);
        assert_eq!(c.max_log_files, 5);
        assert_eq!(c.smtp_port, 25);
        assert_eq!(c.task_name, "ProcDump Monitor");
    }

    #[test]
    fn json_field_names_are_csharp_pascal_case() {
        let mut c = Config::default();
        c.memory_commit_mb = 2048;
        c.dump_retention_max_gb = 1.5;
        let json = serde_json::to_string_pretty(&c).unwrap();
        for key in ["\"ConfigVersion\"", "\"TargetName\"", "\"TargetType\"",
                    "\"ProcDumpPath\"", "\"MemoryCommitMB\"", "\"MinFreeDiskMB\"",
                    "\"MaxLogSizeMB\"", "\"DumpRetentionMaxGB\"", "\"UseSsl\"",
                    "\"EncryptedPasswordBlob\"", "\"EncryptedWebhookUrlBlob\""] {
            assert!(json.contains(key), "missing {key} in: {json}");
        }
    }

    #[test]
    fn round_trip_and_load_of_missing_or_bad_file() {
        let dir = std::env::temp_dir().join("pdm_cfg_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.json");
        let mut c = Config::default();
        c.target_name = "notepad".into();
        c.target_type = TargetType::Service;
        c.save(&p).unwrap();
        let loaded = Config::load(&p);
        assert_eq!(loaded.target_name, "notepad");
        assert_eq!(loaded.target_type, TargetType::Service);
        // missing file -> defaults
        assert_eq!(Config::load(&dir.join("nope.json")).scenario, "Crash capture");
        // corrupt file -> defaults (C# behavior)
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(Config::load(&p).scenario, "Crash capture");
    }
}

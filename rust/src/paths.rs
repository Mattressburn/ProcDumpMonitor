#![allow(dead_code)] // consumed from Task 3 onward

use std::path::PathBuf;

/// Directory containing the real on-disk exe. All portable data lives here.
pub fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| install_dir().join("ProcDumpMonitor.exe"))
}

pub fn config_path() -> PathBuf { install_dir().join("config.json") }
pub fn health_path() -> PathBuf { install_dir().join("health.json") }

pub fn log_dir() -> PathBuf {
    let d = install_dir().join("Logs");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn log_path() -> PathBuf { log_dir().join("procdump.log") }

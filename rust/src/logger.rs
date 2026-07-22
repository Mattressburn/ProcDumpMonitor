use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

struct LogState { path: PathBuf, max_bytes: u64, max_files: i32 }
static STATE: Mutex<Option<LogState>> = Mutex::new(None);

pub fn init(path: PathBuf, max_size_mb: i32, max_files: i32) {
    *STATE.lock().unwrap() = Some(LogState {
        path,
        max_bytes: (max_size_mb.max(0) as u64) * 1024 * 1024,
        max_files,
    });
}

/// Never panics, never throws — logging must not crash the monitor.
pub fn log(category: &str, msg: &str) {
    let guard = match STATE.lock() { Ok(g) => g, Err(_) => return };
    let Some(st) = guard.as_ref() else { return };
    let line = format!("[{}] [{}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), category, msg);
    rotate_if_needed(st);
    if let Ok(mut f) = std::fs::File::options().create(true).append(true).open(&st.path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// procdump.log -> .1 -> .2 -> ... -> .N (oldest dropped).
fn rotate_if_needed(st: &LogState) {
    if st.max_bytes == 0 || st.max_files <= 0 { return; }
    let Ok(meta) = std::fs::metadata(&st.path) else { return };
    if meta.len() < st.max_bytes { return; }
    let p = st.path.display().to_string();
    let _ = std::fs::remove_file(format!("{p}.{}", st.max_files));
    for i in (1..st.max_files).rev() {
        let _ = std::fs::rename(format!("{p}.{i}"), format!("{p}.{}", i + 1));
    }
    let _ = std::fs::rename(&st.path, format!("{p}.1"));
}

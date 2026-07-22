use crate::logger;
use std::path::Path;
use std::time::{Duration, Instant};

/// Size unchanged for 2 consecutive polls AND (windows) exclusive-open succeeds.
pub fn wait_for_stable_file(path: &Path, timeout_s: i32, poll_s: i32) -> bool {
    if !path.exists() { return false; }
    let timeout = if timeout_s <= 0 { 30 } else { timeout_s } as u64;
    let poll = if poll_s <= 0 { 2 } else { poll_s } as u64;
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut last_size: i64 = -1;
    let mut stable_polls = 0;

    while Instant::now() < deadline {
        match std::fs::metadata(path) {
            Ok(m) => {
                let size = m.len() as i64;
                if size == last_size && size > 0 { stable_polls += 1 } else { stable_polls = 0 }
                last_size = size;
                if stable_polls >= 1 && can_open_exclusive(path) {
                    return true;
                }
            }
            Err(_) => return false,
        }
        std::thread::sleep(Duration::from_secs(poll));
    }
    logger::log("Stability", &format!("Timeout ({timeout}s) waiting for stable file: {}", path.display()));
    false
}

#[cfg(windows)]
fn can_open_exclusive(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::File::options().read(true).share_mode(0).open(path).is_ok()
}

#[cfg(not(windows))]
fn can_open_exclusive(_path: &Path) -> bool {
    true // ponytail: POSIX has no mandatory locks; size stability is the whole check in tests
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_file_returns_true_quickly() {
        let dir = std::env::temp_dir().join("pdm_stab");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("done.dmp");
        std::fs::write(&p, b"complete dump").unwrap();
        assert!(wait_for_stable_file(&p, 10, 1));
    }

    #[test]
    fn missing_file_returns_false() {
        assert!(!wait_for_stable_file(std::path::Path::new("/nonexistent.dmp"), 1, 1));
    }

    #[test]
    fn growing_file_times_out() {
        let dir = std::env::temp_dir().join("pdm_stab_grow");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("grow.dmp");
        std::fs::write(&p, b"x").unwrap();
        let p2 = p.clone();
        let grower = std::thread::spawn(move || {
            for _ in 0..6 {
                std::thread::sleep(std::time::Duration::from_millis(900));
                use std::io::Write;
                let mut f = std::fs::File::options().append(true).open(&p2).unwrap();
                let _ = f.write_all(&[0u8; 64]);
            }
        });
        let stable = wait_for_stable_file(&p, 4, 1);
        grower.join().unwrap();
        assert!(!stable, "file growing for the whole window must not be stable");
    }
}

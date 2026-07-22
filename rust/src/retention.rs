use crate::logger;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Age policy then size policy, oldest-first. 0 disables each. Returns deletions.
pub fn apply(dump_dir: &Path, retention_days: i32, max_gb: f64) -> usize {
    if retention_days <= 0 && max_gb <= 0.0 { return 0; }
    let Ok(rd) = std::fs::read_dir(dump_dir) else { return 0 };

    // (path, mtime, size), .dmp only, oldest first
    let mut files: Vec<(std::path::PathBuf, SystemTime, u64)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("dmp")))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((e.path(), m.modified().ok()?, m.len()))
        })
        .collect();
    files.sort_by_key(|(_, t, _)| *t);

    let mut deleted = 0usize;

    if retention_days > 0 {
        let cutoff = SystemTime::now() - Duration::from_secs(retention_days as u64 * 86400);
        files.retain(|(p, t, _)| {
            if *t < cutoff && std::fs::remove_file(p).is_ok() {
                deleted += 1;
                logger::log("Retention", &format!("Deleted aged dump ({retention_days}d policy): {}", p.display()));
                false
            } else { true }
        });
    }

    if max_gb > 0.0 {
        let max_bytes = (max_gb * 1024.0 * 1024.0 * 1024.0) as u64;
        let mut total: u64 = files.iter().map(|(_, _, s)| s).sum();
        for (p, _, size) in &files {
            if total <= max_bytes { break; }
            if std::fs::remove_file(p).is_ok() {
                total -= size;
                deleted += 1;
                logger::log("Retention", &format!("Deleted dump (over {max_gb:.1} GB cap): {}", p.display()));
            }
        }
    }

    deleted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn mk(dir: &std::path::Path, name: &str, size: usize, age_days: u64) {
        let p = dir.join(name);
        std::fs::write(&p, vec![0u8; size]).unwrap();
        let t = SystemTime::now() - Duration::from_secs(age_days * 86400);
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
    }

    #[test]
    fn age_policy_deletes_only_old_dmp() {
        let dir = std::env::temp_dir().join("pdm_ret_age");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        mk(&dir, "old.dmp", 10, 10);
        mk(&dir, "new.dmp", 10, 1);
        mk(&dir, "old.txt", 10, 10); // non-dmp untouched
        assert_eq!(apply(&dir, 7, 0.0), 1);
        assert!(!dir.join("old.dmp").exists());
        assert!(dir.join("new.dmp").exists() && dir.join("old.txt").exists());
    }

    #[test]
    fn size_policy_deletes_oldest_first_until_under_cap() {
        let dir = std::env::temp_dir().join("pdm_ret_size");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // cap = 2 MB; three 1 MB files, oldest two must go? total 3MB -> delete oldest -> 2MB = at cap, stop
        mk(&dir, "a.dmp", 1_048_576, 3);
        mk(&dir, "b.dmp", 1_048_576, 2);
        mk(&dir, "c.dmp", 1_048_576, 1);
        let cap_gb = 2.0 / 1024.0;
        assert_eq!(apply(&dir, 0, cap_gb), 1);
        assert!(!dir.join("a.dmp").exists());
        assert!(dir.join("b.dmp").exists() && dir.join("c.dmp").exists());
    }

    #[test]
    fn disabled_policies_do_nothing() {
        let dir = std::env::temp_dir().join("pdm_ret_off");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        mk(&dir, "x.dmp", 10, 100);
        assert_eq!(apply(&dir, 0, 0.0), 0);
        assert!(dir.join("x.dmp").exists());
    }
}

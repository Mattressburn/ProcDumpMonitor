// ponytail: check_free_space is only called from monitor.rs, which is
// #[cfg(windows)] — this product's entry points are Windows-only, so the
// non-windows shadow impl below has no Linux caller outside tests.
#![cfg_attr(not(windows), allow(dead_code))]

/// (ok, free_mb). min<=0 disables (true, 0). Fails open on error (true, -1).
#[cfg(windows)]
pub fn check_free_space(path: &std::path::Path, min_free_mb: i64) -> (bool, i64) {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    if min_free_mb <= 0 { return (true, 0); }
    let mut free: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(&HSTRING::from(path.as_os_str()), Some(&mut free), None, None)
    };
    match ok {
        Ok(()) => {
            let free_mb = (free / (1024 * 1024)) as i64;
            (free_mb >= min_free_mb, free_mb)
        }
        Err(_) => (true, -1),
    }
}

#[cfg(not(windows))]
pub fn check_free_space(_path: &std::path::Path, min_free_mb: i64) -> (bool, i64) {
    if min_free_mb <= 0 { (true, 0) } else { (true, -1) }
}

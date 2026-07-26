//! CCURE install discovery — ports the PS1's Get-SwhInstallLocation /
//! Get-VendorRootsFromInstallLocation / Get-DefaultLogComponents. Pure
//! functions take the registry text / an `exists` probe so they unit-test on
//! any host; only `install_location()` spawns reg.exe.

use std::path::{Path, PathBuf};

pub const SWH_SETUP_KEY: &str =
    r"HKLM\SOFTWARE\WOW6432Node\Sensormatic Electronics Corporation\SWHSystem\Setup";

/// Parse `reg.exe query <key> /v InstallLocation` output:
/// `    InstallLocation    REG_SZ    C:\Program Files (x86)\Tyco`
pub fn parse_reg_value(out: &str, value_name: &str) -> Option<String> {
    for line in out.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix(value_name) {
            // "REG_SZ" (or REG_EXPAND_SZ) then the data, whitespace-separated.
            let rest = rest.trim_start();
            if let Some(reg_pos) = rest.find("REG_") {
                let after_type = &rest[reg_pos..];
                if let Some(sp) = after_type.find(char::is_whitespace) {
                    let val = after_type[sp..].trim();
                    if !val.is_empty() {
                        return Some(val.to_string());
                    }
                }
            }
        }
    }
    None
}

/// JCI + Tyco vendor roots derived from InstallLocation (PS1 logic: root
/// markers first, then a \JCI\ or \Tyco\ path segment, then subdirs, then
/// Program Files (x86) fallbacks on the install drive).
pub fn vendor_roots(
    install_location: Option<&str>,
    exists: &dyn Fn(&Path) -> bool,
) -> (PathBuf, PathBuf) {
    let fallback = |root: &str| {
        (
            PathBuf::from(root).join(r"Program Files (x86)\JCI"),
            PathBuf::from(root).join(r"Program Files (x86)\Tyco"),
        )
    };
    let Some(loc) = install_location.map(str::trim).filter(|s| !s.is_empty()) else {
        return fallback("C:\\");
    };
    let norm = PathBuf::from(loc.trim_end_matches('\\'));

    // Marker dirs directly under InstallLocation -> it IS the vendor root.
    const MARKERS: [&str; 6] = [
        "CrossFire", "CCure Portal", "CCure Web", "SecurityIntelligence", "victorWeb",
        "victorWebServices",
    ];
    if MARKERS.iter().any(|m| exists(&norm.join(m))) {
        return (norm.clone(), norm);
    }

    // A \JCI\ or \Tyco\ segment anywhere in the path.
    let comps: Vec<String> =
        norm.iter().map(|c| c.to_string_lossy().to_string()).collect();
    for (i, c) in comps.iter().enumerate() {
        let is_jci = c.eq_ignore_ascii_case("JCI");
        let is_tyco = c.eq_ignore_ascii_case("Tyco");
        if is_jci || is_tyco {
            let base: PathBuf = comps[..i].iter().collect();
            let this = base.join(c);
            let other_name = if is_jci { "Tyco" } else { "JCI" };
            let other = base.join(other_name);
            let other = if exists(&other) { other } else { this.clone() };
            return if is_jci { (this, other) } else { (other, this) };
        }
    }

    // JCI/Tyco subdirectories of InstallLocation.
    let sub_jci = norm.join("JCI");
    let sub_tyco = norm.join("Tyco");
    let drive = {
        let mut it = norm.iter();
        it.next().map(|p| PathBuf::from(p).join("\\")).unwrap_or_else(|| PathBuf::from("C:\\"))
    };
    let (fb_jci, fb_tyco) = fallback(&drive.to_string_lossy());
    (
        if exists(&sub_jci) { sub_jci } else { fb_jci },
        if exists(&sub_tyco) { sub_tyco } else { fb_tyco },
    )
}

/// The PS1's default log components: (display name, relative path).
pub const LOG_COMPONENTS: [(&str, &str); 7] = [
    ("CCure Portal logs", r"CCure Portal\logs"),
    ("CCure Web logs", r"CCure Web\logs"),
    ("CrossFire Logging (System Trace)", r"CrossFire\Logging"),
    ("Security Intelligence Datacache logs", r"SecurityIntelligence\Datacache\logs"),
    ("VictorWeb Logs", r"victorWeb\Logs"),
    ("VictorWebServices auth logs", r"victorWebServices\victorWebsite\auth\logs"),
    ("VictorWebServices Website Logs", r"victorWebServices\victorWebsite\Logs"),
];

/// Per component: unique candidate paths under both vendor roots.
pub fn log_component_paths(jci: &Path, tyco: &Path) -> Vec<(String, Vec<PathBuf>)> {
    LOG_COMPONENTS
        .iter()
        .map(|(name, rel)| {
            let mut paths = vec![jci.join(rel)];
            let t = tyco.join(rel);
            if t != paths[0] {
                paths.push(t);
            }
            (name.to_string(), paths)
        })
        .collect()
}

/// Newest InstallHistory.xml under ProgramData\{Tyco,JCI}\InstallerTemp.
pub fn find_install_history(program_data: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for vendor in ["Tyco", "JCI"] {
        let root = program_data.join(vendor).join("InstallerTemp");
        walk_for(&root, "InstallHistory.xml", 4, &mut |p, mtime| {
            if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                best = Some((mtime, p.to_path_buf()));
            }
        });
    }
    best.map(|(_, p)| p)
}

/// Newest `Dashboard.exe.config` files under ProgramData (SWHSystem settings
/// source; the bundle copies the files verbatim rather than parsing the XML).
pub fn find_dashboard_configs(program_data: &Path, max: usize) -> Vec<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    walk_for(program_data, "Dashboard.exe.config", 4, &mut |p, mtime| {
        found.push((mtime, p.to_path_buf()));
    });
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().take(max).map(|(_, p)| p).collect()
}

/// Bounded-depth recursive search for an exact (case-insensitive) file name.
fn walk_for(
    dir: &Path,
    file_name: &str,
    depth: u32,
    hit: &mut dyn FnMut(&Path, std::time::SystemTime),
) {
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_for(&path, file_name, depth - 1, hit);
        } else if path
            .file_name()
            .map(|n| n.to_string_lossy().eq_ignore_ascii_case(file_name))
            .unwrap_or(false)
        {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            hit(&path, mtime);
        }
    }
}

/// InstallCache dirs (JCI preferred, Tyco fallback — PS1 order).
pub fn install_cache_candidates(program_data: &Path) -> [PathBuf; 2] {
    [
        program_data.join(r"JCI\InstallCache"),
        program_data.join(r"Tyco\InstallCache"),
    ]
}

/// Bulk-update candidate dirs (CrossFire\ServerComponents under install
/// location, its parent, both vendor roots, and PF(x86) fallbacks).
pub fn bulk_update_candidates(
    install_location: Option<&str>,
    jci: &Path,
    tyco: &Path,
) -> Vec<PathBuf> {
    const REL: &str = r"CrossFire\ServerComponents";
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(loc) = install_location.map(str::trim).filter(|s| !s.is_empty()) {
        let norm = PathBuf::from(loc.trim_end_matches('\\'));
        dirs.push(norm.join(REL));
        if let Some(parent) = norm.parent() {
            dirs.push(parent.join(REL));
        }
    }
    dirs.push(jci.join(REL));
    dirs.push(tyco.join(REL));
    dirs.push(PathBuf::from(r"C:\Program Files (x86)\JCI").join(REL));
    dirs.push(PathBuf::from(r"C:\Program Files (x86)\Tyco").join(REL));
    dirs.dedup();
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.to_string_lossy().to_lowercase()));
    dirs
}

/// Live registry read of the CCURE InstallLocation.
#[cfg(windows)]
pub fn install_location() -> Option<String> {
    let out = super::run_tool("reg.exe", &["query", SWH_SETUP_KEY, "/v", "InstallLocation"]).ok()?;
    parse_reg_value(&String::from_utf8_lossy(&out.stdout), "InstallLocation")
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG_SAMPLE: &str = "\r\n\
HKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\Sensormatic Electronics Corporation\\SWHSystem\\Setup\r\n\
    InstallLocation    REG_SZ    C:\\Program Files (x86)\\Tyco\r\n\r\n";

    #[test]
    fn parses_reg_query_output() {
        assert_eq!(
            parse_reg_value(REG_SAMPLE, "InstallLocation").as_deref(),
            Some(r"C:\Program Files (x86)\Tyco")
        );
        assert_eq!(parse_reg_value("", "InstallLocation"), None);
        assert_eq!(parse_reg_value("garbage\nlines", "InstallLocation"), None);
    }

    #[test]
    fn reg_value_with_spaces_in_data_survives() {
        let out = "    InstallLocation    REG_EXPAND_SZ    C:\\My Dir\\Sub Dir\\Tyco\r\n";
        assert_eq!(parse_reg_value(out, "InstallLocation").as_deref(), Some(r"C:\My Dir\Sub Dir\Tyco"));
    }

    #[test]
    fn vendor_roots_marker_dir_means_install_location_is_root() {
        let loc = r"D:\Apps\CCure";
        let exists = |p: &Path| p == Path::new(r"D:\Apps\CCure\CrossFire");
        let (jci, tyco) = vendor_roots(Some(loc), &exists);
        assert_eq!(jci, PathBuf::from(loc));
        assert_eq!(tyco, PathBuf::from(loc));
    }

    #[test]
    fn vendor_roots_tyco_segment_finds_sibling_jci() {
        let exists = |p: &Path| p == Path::new(r"C:\Program Files (x86)\JCI");
        let (jci, tyco) =
            vendor_roots(Some(r"C:\Program Files (x86)\Tyco\CrossFire"), &exists);
        assert_eq!(tyco, PathBuf::from(r"C:\Program Files (x86)\Tyco"));
        assert_eq!(jci, PathBuf::from(r"C:\Program Files (x86)\JCI"));
    }

    #[test]
    fn vendor_roots_tyco_segment_without_sibling_uses_tyco_for_both() {
        let exists = |_: &Path| false;
        let (jci, tyco) = vendor_roots(Some(r"E:\Tyco\CrossFire"), &exists);
        assert_eq!(tyco, PathBuf::from(r"E:\Tyco"));
        assert_eq!(jci, PathBuf::from(r"E:\Tyco"));
    }

    #[test]
    fn vendor_roots_none_falls_back_to_pf86() {
        let exists = |_: &Path| false;
        let (jci, tyco) = vendor_roots(None, &exists);
        assert_eq!(jci, PathBuf::from(r"C:\Program Files (x86)\JCI"));
        assert_eq!(tyco, PathBuf::from(r"C:\Program Files (x86)\Tyco"));
    }

    #[test]
    fn log_component_paths_dedupe_when_roots_equal() {
        let root = PathBuf::from(r"D:\CCure");
        let comps = log_component_paths(&root, &root);
        assert_eq!(comps.len(), 7);
        assert!(comps.iter().all(|(_, p)| p.len() == 1));
        let comps2 = log_component_paths(
            &PathBuf::from(r"C:\PF\JCI"),
            &PathBuf::from(r"C:\PF\Tyco"),
        );
        assert!(comps2.iter().all(|(_, p)| p.len() == 2));
    }

    #[test]
    fn bulk_update_candidates_unique_case_insensitive() {
        let dirs = bulk_update_candidates(
            Some(r"C:\Program Files (x86)\Tyco"),
            Path::new(r"C:\Program Files (x86)\JCI"),
            Path::new(r"C:\Program Files (x86)\Tyco"),
        );
        let set: std::collections::HashSet<String> =
            dirs.iter().map(|d| d.to_string_lossy().to_lowercase()).collect();
        assert_eq!(set.len(), dirs.len());
    }

    #[test]
    fn find_install_history_picks_newest() {
        let base = std::env::temp_dir().join("pdm_discover_test");
        let _ = std::fs::remove_dir_all(&base);
        let old_dir = base.join(r"Tyco\InstallerTemp\a");
        let new_dir = base.join(r"JCI\InstallerTemp");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::create_dir_all(&new_dir).unwrap();
        std::fs::write(old_dir.join("InstallHistory.xml"), "old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(new_dir.join("InstallHistory.xml"), "new").unwrap();
        let found = find_install_history(&base).unwrap();
        assert_eq!(std::fs::read_to_string(found).unwrap(), "new");
    }
}

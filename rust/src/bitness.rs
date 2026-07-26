// ponytail: BinaryChoice.summary is only read from monitor.rs, which is
// #[cfg(windows)] — this product's entry points are Windows-only.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::{Path, PathBuf};

const IMAGE_FILE_MACHINE_I386: u16 = 0x014C;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bitness { Unknown, X86, X64 }

/// Read `IMAGE_FILE_HEADER.Machine` from a PE on disk.
///
/// This is what makes `-w` (wait-for-process) correct: it needs no running
/// process, so a target that has never started still resolves.
///
/// Layout: `e_lfanew` is a LE u32 at 0x3C pointing at the PE signature
/// "PE\0\0"; `IMAGE_FILE_HEADER` follows it and opens with a LE u16 Machine.
pub fn pe_machine(path: &Path) -> Option<u16> {
    use std::io::{Read, Seek, SeekFrom};

    let mut f = std::fs::File::open(path).ok()?;

    let mut lfanew = [0u8; 4];
    f.seek(SeekFrom::Start(0x3C)).ok()?;
    f.read_exact(&mut lfanew).ok()?;
    let off = u32::from_le_bytes(lfanew) as u64;

    // Guard against a hostile/garbage offset before seeking.
    let len = f.metadata().ok()?.len();
    if off.checked_add(6)? > len {
        return None;
    }

    let mut head = [0u8; 6]; // "PE\0\0" + Machine
    f.seek(SeekFrom::Start(off)).ok()?;
    f.read_exact(&mut head).ok()?;
    if &head[0..4] != b"PE\0\0" {
        return None;
    }
    Some(u16::from_le_bytes([head[4], head[5]]))
}

/// Map a PE machine value to the binary-selection decision.
///
/// NOTE: ARM64 and AMD64 both map to X64. That is correct for choosing a
/// ProcDump binary, but it means the two are indistinguishable here — do not
/// assert anything stronger.
pub fn bitness_from_pe(path: &Path) -> Bitness {
    match pe_machine(path) {
        Some(IMAGE_FILE_MACHINE_I386) => Bitness::X86,
        Some(IMAGE_FILE_MACHINE_AMD64) | Some(IMAGE_FILE_MACHINE_ARM64) => Bitness::X64,
        _ => Bitness::Unknown,
    }
}

/// Turn a raw registry `ImagePath` into a usable file path.
///
/// Handles: quoted paths, trailing arguments, the `\??\` NT prefix, and
/// embedded environment variables. Pure — no registry access, so it is
/// directly unit-testable.
pub fn parse_image_path(raw: &str) -> Option<PathBuf> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    // Quoted form is unambiguous: take what is inside the quotes.
    let path = if let Some(rest) = s.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        // Unquoted: split after the first ".exe" token.
        // ponytail: ambiguous in theory (an unquoted path containing spaces AND
        // arguments, or a directory literally named `...exe...`, e.g.
        // `C:\App.exe.bak\svc.exe`, which truncates wrong), but this is what
        // well-formed service entries look like. Upgrade path: probe candidate
        // prefixes against the filesystem, if a real ImagePath ever breaks it.
        //
        // to_ascii_lowercase is load-bearing, do NOT "modernize" it to
        // to_lowercase(): ASCII folding maps 0x41..=0x5A one byte to one byte
        // and leaves every byte >= 0x80 alone, so byte indices and char
        // boundaries are identical to the original and `s[..i + 4]` cannot
        // panic. Unicode to_lowercase() can change byte length (U+0130 'İ' is
        // 2 bytes, lowercases to 3), shifting indices into mid-codepoint.
        match s.to_ascii_lowercase().find(".exe") {
            Some(i) => s[..i + 4].to_string(),
            None => s.to_string(),
        }
    };

    let path = path.trim().trim_start_matches(r"\??\").trim();
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(expand_env(path)))
}

/// Expand %VAR% tokens. std has no equivalent and we add no crates.
fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    // Unknown var: keep it literal rather than silently
                    // producing a wrong path.
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push('%');
                out.push_str(after);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub struct BinaryChoice {
    pub actual: PathBuf,
    pub warning: Option<String>,
    pub summary: String,
}

/// Pure binary selection — port of C# SelectBinary, os_is_64 injected for tests.
pub fn select_binary(bitness: Bitness, procdump_dir: &Path, os_is_64: bool) -> BinaryChoice {
    let pd64 = procdump_dir.join("procdump64.exe");
    let pd32 = procdump_dir.join("procdump.exe");
    let has64 = pd64.exists();
    let has32 = pd32.exists();

    if !has64 && !has32 {
        return BinaryChoice {
            actual: PathBuf::new(),
            warning: Some("Neither procdump.exe nor procdump64.exe found in the ProcDump directory.".into()),
            summary: "No ProcDump binary found".into(),
        };
    }

    if !os_is_64 {
        return BinaryChoice {
            actual: if has32 { pd32 } else { pd64 },
            warning: if has32 { None } else {
                Some("procdump.exe not found; using procdump64.exe but it may not work on a 32-bit OS.".into())
            },
            summary: "32-bit OS -> procdump.exe".into(),
        };
    }

    match bitness {
        Bitness::X86 => {
            if has32 {
                BinaryChoice { actual: pd32, warning: None, summary: "32-bit process -> procdump.exe".into() }
            } else {
                BinaryChoice {
                    actual: pd64,
                    warning: Some("procdump.exe not found - falling back to procdump64.exe.".into()),
                    summary: "32-bit process -> procdump64.exe (fallback)".into(),
                }
            }
        }
        Bitness::X64 => {
            if has64 {
                BinaryChoice { actual: pd64, warning: None, summary: "64-bit process -> procdump64.exe".into() }
            } else {
                BinaryChoice {
                    actual: pd32,
                    warning: Some("procdump64.exe not found - falling back to procdump.exe.".into()),
                    summary: "64-bit process -> procdump.exe (fallback)".into(),
                }
            }
        }
        Bitness::Unknown => BinaryChoice {
            actual: if has64 { pd64 } else { pd32 },
            warning: if has64 { None } else { Some("procdump64.exe not found; using procdump.exe as fallback.".into()) },
            summary: if has64 { "Unknown bitness -> procdump64.exe (default)".into() }
                     else { "Unknown bitness -> procdump.exe".into() },
        },
    }
}

/// Unique running-process exe names via Toolhelp, sorted case-insensitively.
/// Feeds the Monitor page's combined Svc:/Proc: target dropdown.
#[cfg(windows)]
pub fn list_process_names() -> Vec<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut names: Vec<String> = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return names;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile
                        [..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)],
                );
                if !name.is_empty() {
                    names.push(name);
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    names.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

/// Find a PID by exe name (case-insensitive, .exe optional) via Toolhelp,
/// then classify with IsWow64Process2 resolved via GetProcAddress.
/// CRITICAL: IsWow64Process2 does not exist on Server 2016 (build 14393) —
/// a static windows-crate import would make the exe FAIL TO LOAD there.
/// (Confirmed: windows 0.58's `windows_targets::link!` uses raw-dylib import
/// linkage, so any statically referenced Win32 symbol becomes a hard import-
/// table entry the OS loader resolves at process load — an absent symbol
/// fails the whole process load, not just the call. GetProcAddress avoids
/// this because a missing export just yields None.)
#[cfg(windows)]
pub fn detect(process_name: &str) -> Bitness {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
        PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let want = process_name.trim_end_matches(".exe").trim_end_matches(".EXE").to_ascii_lowercase();

    let pid = unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return Bitness::Unknown;
        };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = 0u32;
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)]);
                if name.trim_end_matches(".exe").eq_ignore_ascii_case(&want) {
                    found = entry.th32ProcessID;
                    break;
                }
                if Process32NextW(snap, &mut entry).is_err() { break; }
            }
        }
        let _ = CloseHandle(snap);
        found
    };
    if pid == 0 { return Bitness::Unknown; }

    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return Bitness::Unknown;
        };
        let result = classify(h);
        let _ = CloseHandle(h);
        result
    }
}

#[cfg(windows)]
unsafe fn classify(h: windows::Win32::Foundation::HANDLE) -> Bitness {
    use windows::core::s;
    use windows::Win32::Foundation::{BOOL, HANDLE};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows::Win32::System::Threading::IsWow64Process;

    // Dynamic lookup — Win10 1607+ only, absent on Server 2016 RTM (14393).
    type IsWow64Process2Fn = unsafe extern "system" fn(HANDLE, *mut u16, *mut u16) -> BOOL;
    if let Ok(kernel32) = GetModuleHandleA(s!("kernel32.dll")) {
        if let Some(f) = GetProcAddress(kernel32, s!("IsWow64Process2")) {
            let f: IsWow64Process2Fn = std::mem::transmute(f);
            let (mut proc_machine, mut native_machine) = (0u16, 0u16);
            if f(h, &mut proc_machine, &mut native_machine).as_bool() {
                if proc_machine == IMAGE_FILE_MACHINE_I386 { return Bitness::X86; }
                if native_machine == IMAGE_FILE_MACHINE_AMD64 || native_machine == IMAGE_FILE_MACHINE_ARM64 {
                    return Bitness::X64;
                }
                return Bitness::X86;
            }
        }
    }
    // Fallback: IsWow64Process (all 64-bit Windows)
    let mut wow64 = BOOL(0);
    if IsWow64Process(h, &mut wow64).is_ok() {
        return if wow64.as_bool() { Bitness::X86 } else { Bitness::X64 };
    }
    Bitness::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[&str]) -> std::path::PathBuf {
        // ponytail: brief's original keyed the dir only on files.join("_").len(),
        // which collides across the 4 tests sharing the same file list and races
        // under cargo's default parallel test threads. Counter makes each call unique.
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("pdm_bit_{}_{n}", files.join("_").len()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for f in files { std::fs::write(d.join(f), b"x").unwrap(); }
        d
    }

    #[test]
    fn x86_target_prefers_procdump_exe() {
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        let c = select_binary(Bitness::X86, &d, true);
        assert!(c.actual.ends_with("procdump.exe"));
        assert!(c.warning.is_none());
    }

    #[test]
    fn x64_target_prefers_procdump64() {
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        assert!(select_binary(Bitness::X64, &d, true).actual.ends_with("procdump64.exe"));
    }

    #[test]
    fn missing_preferred_falls_back_with_warning() {
        let d = dir_with(&["procdump64.exe"]);
        let c = select_binary(Bitness::X86, &d, true);
        assert!(c.actual.ends_with("procdump64.exe"));
        assert!(c.warning.is_some());
    }

    #[test]
    fn unknown_defaults_to_64_on_64bit_os() {
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        assert!(select_binary(Bitness::Unknown, &d, true).actual.ends_with("procdump64.exe"));
    }

    #[test]
    fn neither_binary_is_reported() {
        let d = dir_with(&[]);
        let c = select_binary(Bitness::X64, &d, true);
        assert_eq!(c.actual, std::path::PathBuf::new());
        assert!(c.warning.is_some());
    }

    #[test]
    fn on_32bit_os_only_procdump_exe() {
        let d = dir_with(&["procdump.exe", "procdump64.exe"]);
        assert!(select_binary(Bitness::X64, &d, false).actual.ends_with("procdump.exe"));
    }

    #[test]
    fn pe_machine_reads_our_own_exe_as_64bit() {
        // current_exe() is the test binary itself: a guaranteed-present x64 PE
        // with no system-path assumption.
        let me = std::env::current_exe().unwrap();
        assert_eq!(pe_machine(&me), Some(0x8664));
        assert_eq!(bitness_from_pe(&me), Bitness::X64);
    }

    #[test]
    fn pe_machine_reads_wow64_binary_as_32bit() {
        // ponytail: SysWOW64 contents vary on ARM64 hosts, so skip rather than
        // fail if absent. cmd.exe is chosen over notepad.exe because notepad is
        // being replaced by the Store package and may be a stub or missing.
        let p = std::path::PathBuf::from(r"C:\Windows\SysWOW64\cmd.exe");
        if !p.exists() {
            eprintln!("skipping: {} not present on this host", p.display());
            return;
        }
        assert_eq!(pe_machine(&p), Some(0x014C));
        assert_eq!(bitness_from_pe(&p), Bitness::X86);
    }

    #[test]
    fn pe_machine_rejects_non_pe_and_missing_files() {
        let d = std::env::temp_dir().join("pdm_pe_notpe.txt");
        std::fs::write(&d, b"this is not a PE file at all, not even close").unwrap();
        assert_eq!(pe_machine(&d), None);
        assert_eq!(bitness_from_pe(&d), Bitness::Unknown);
        let _ = std::fs::remove_file(&d);

        let missing = std::env::temp_dir().join("pdm_pe_does_not_exist.exe");
        assert_eq!(pe_machine(&missing), None);
    }

    #[test]
    fn pe_machine_rejects_truncated_file() {
        // A file shorter than the DOS header must not panic or index out of range.
        let d = std::env::temp_dir().join("pdm_pe_short.bin");
        std::fs::write(&d, b"MZ").unwrap();
        assert_eq!(pe_machine(&d), None);
        let _ = std::fs::remove_file(&d);
    }

    /// A 64-byte DOS header: "MZ" at 0, `e_lfanew` as a LE u32 at 0x3C.
    /// Long enough that the `e_lfanew` read succeeds, so execution reaches
    /// the length guard and the signature check — which the short/text
    /// fixtures above never do (they die at `read_exact` on a past-EOF seek).
    fn dos_header(lfanew: u32) -> Vec<u8> {
        let mut v = vec![0u8; 64];
        v[0..2].copy_from_slice(b"MZ");
        v[0x3C..0x40].copy_from_slice(&lfanew.to_le_bytes());
        v
    }

    #[test]
    fn pe_machine_rejects_garbage_lfanew_past_eof() {
        // Reaches `off.checked_add(6)? > len` with a garbage offset past the end
        // of a 64-byte file. ponytail: this pins the BEHAVIOUR, not the guard —
        // mutation-tested, and deleting the guard still passes, because if
        // off + 6 > len then read_exact of 6 bytes at off must hit EOF anyway.
        // The guard is unfalsifiable-by-construction defense in depth at a
        // file-parse trust boundary; kept deliberately. Nothing to add here.
        let d = std::env::temp_dir().join("pdm_pe_badoff.bin");
        std::fs::write(&d, dos_header(0xFFFF_FFF0)).unwrap();
        assert_eq!(pe_machine(&d), None);
        let _ = std::fs::remove_file(&d);
    }

    #[test]
    fn pe_machine_rejects_bad_signature() {
        // Covers `if &head[0..4] != b"PE\0\0"`: e_lfanew is in range and the
        // 6 bytes there are readable, but the signature is wrong. 70 bytes so
        // off(0x40) + 6 == len and the length guard passes.
        let d = std::env::temp_dir().join("pdm_pe_badsig.bin");
        let mut v = dos_header(0x40);
        v.extend_from_slice(b"XX\0\0"); // where "PE\0\0" belongs
        v.extend_from_slice(&[0x64, 0x86]); // a valid-looking AMD64 Machine
        std::fs::write(&d, &v).unwrap();
        assert_eq!(pe_machine(&d), None);
        let _ = std::fs::remove_file(&d);
    }

    #[test]
    fn image_path_plain_unquoted() {
        assert_eq!(
            parse_image_path(r"C:\Program Files\SWH\CrossFire.Server.exe"),
            Some(std::path::PathBuf::from(r"C:\Program Files\SWH\CrossFire.Server.exe"))
        );
    }

    #[test]
    fn image_path_quoted_with_arguments() {
        // Quoted form is unambiguous: everything inside the quotes is the path.
        assert_eq!(
            parse_image_path(r#""C:\Program Files\SWH\CrossFire.Server.exe" -k netsvcs"#),
            Some(std::path::PathBuf::from(r"C:\Program Files\SWH\CrossFire.Server.exe"))
        );
    }

    #[test]
    fn image_path_unquoted_with_arguments_splits_at_exe() {
        // Unquoted with spaces AND args is genuinely ambiguous; split after the
        // first ".exe" token, which is what Windows itself effectively does for
        // well-formed service entries.
        assert_eq!(
            parse_image_path(r"C:\Windows\system32\svchost.exe -k netsvcs"),
            Some(std::path::PathBuf::from(r"C:\Windows\system32\svchost.exe"))
        );
    }

    #[test]
    fn image_path_strips_nt_prefix() {
        assert_eq!(
            parse_image_path(r"\??\C:\Program Files\SWH\Driver.exe"),
            Some(std::path::PathBuf::from(r"C:\Program Files\SWH\Driver.exe"))
        );
    }

    #[test]
    fn image_path_uppercase_exe_keeps_original_casing() {
        // Guards the `to_ascii_lowercase()` in the unquoted branch. It exists
        // ONLY to find the token; the slice is taken from the ORIGINAL, so
        // casing survives. A plain `s.find(".exe")` misses this entirely and
        // returns the arguments too — which is the "modernization" the code
        // comment there warns about.
        assert_eq!(
            parse_image_path(r"C:\WINDOWS\SYSTEM32\SVCHOST.EXE -k netsvcs"),
            Some(std::path::PathBuf::from(r"C:\WINDOWS\SYSTEM32\SVCHOST.EXE"))
        );
    }

    #[test]
    fn image_path_expands_environment_variables() {
        // %SystemRoot% is set on every Windows host. Assert the ACTUAL expanded
        // value: "no % remains and it ends with the suffix" would also pass for
        // an expand_env that simply deleted the token.
        assert_eq!(
            parse_image_path(r"%SystemRoot%\system32\services.exe"),
            Some(std::path::PathBuf::from(
                std::env::var("SystemRoot").unwrap() + r"\system32\services.exe"
            ))
        );
    }

    #[test]
    fn image_path_keeps_unknown_env_var_literal() {
        // expand_env's Err(_) arm: an unset variable stays literal rather than
        // silently collapsing to a wrong-but-plausible path.
        assert_eq!(
            parse_image_path(r"%PDM_NO_SUCH_VAR%\svc.exe"),
            Some(std::path::PathBuf::from(r"%PDM_NO_SUCH_VAR%\svc.exe"))
        );
    }

    #[test]
    fn image_path_rejects_empty() {
        assert_eq!(parse_image_path(""), None);
        assert_eq!(parse_image_path("   "), None);
    }
}

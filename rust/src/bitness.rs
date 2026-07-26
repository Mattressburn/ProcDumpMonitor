// ponytail: BinaryChoice.summary is only read from monitor.rs, which is
// #[cfg(windows)] — this product's entry points are Windows-only.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::config::{Config, TargetType};
use std::path::{Path, PathBuf};

const IMAGE_FILE_MACHINE_I386: u16 = 0x014C;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;

// COR20 (CLI) header Flags — CorHdr.h. ILONLY (0x1) is deliberately NOT part of
// the predicate: see `bitness_from_pe`.
const COMIMAGE_FLAGS_32BITREQUIRED: u32 = 0x0000_0002;
const COMIMAGE_FLAGS_32BITPREFERRED: u32 = 0x0002_0000;

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

/// Read `len` bytes at `off`. Seeking past EOF is legal on Windows, so the
/// short/truncated case surfaces as `read_exact` failing, not as a panic.
fn read_at(f: &mut std::fs::File, off: u64, buf: &mut [u8]) -> Option<()> {
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(off)).ok()?;
    f.read_exact(buf).ok()?;
    Some(())
}

/// Translate an RVA to a file offset by walking the section table.
///
/// Everything is computed in u64 so `va + raw_size` cannot wrap on a hostile
/// `SizeOfRawData` (both inputs are u32, the sum fits comfortably).
fn rva_to_file_offset(
    f: &mut std::fs::File,
    section_table: u64,
    n_sections: u64,
    rva: u64,
) -> Option<u64> {
    // NumberOfSections is an attacker-controlled u16. The PE spec caps an
    // image at 96 sections; without this cap a malformed file costs 65535
    // seek+read pairs inside the monitor loop.
    for i in 0..n_sections.min(96) {
        // Section header: Name(8) VirtualSize(4) VirtualAddress(4)
        //                 SizeOfRawData(4) PointerToRawData(4) ...
        let mut s = [0u8; 24];
        read_at(f, section_table.checked_add(i.checked_mul(40)?)?, &mut s)?;
        let va = u32::from_le_bytes([s[12], s[13], s[14], s[15]]) as u64;
        let raw_size = u32::from_le_bytes([s[16], s[17], s[18], s[19]]) as u64;
        let raw_ptr = u32::from_le_bytes([s[20], s[21], s[22], s[23]]) as u64;
        if rva >= va && rva < va + raw_size {
            return raw_ptr.checked_add(rva - va);
        }
    }
    None
}

/// `IMAGE_COR20_HEADER.Flags` for a managed image, or None when the file is
/// unmanaged, truncated, or malformed anywhere along the walk.
///
/// PANIC-FREE BY CONSTRUCTION, and that is a hard requirement: `panic = "abort"`
/// means any panic here kills the monitor process. Every byte comes from a
/// fixed-size `read_exact` into a fixed-size array (so no slicing can be out of
/// range), every offset is `checked_add`/`checked_mul`, and every fallible step
/// returns None. There is no indexing by a value read from the file.
fn cor20_flags(path: &Path) -> Option<u32> {
    let mut f = std::fs::File::open(path).ok()?;

    let mut lfanew = [0u8; 4];
    read_at(&mut f, 0x3C, &mut lfanew)?;
    let pe_off = u32::from_le_bytes(lfanew) as u64;

    // "PE\0\0"(4) Machine(2) NumberOfSections(2) TimeDateStamp(4)
    // PointerToSymbolTable(4) NumberOfSymbols(4) SizeOfOptionalHeader(2)
    // Characteristics(2) = 24 bytes.
    let mut coff = [0u8; 24];
    read_at(&mut f, pe_off, &mut coff)?;
    if &coff[0..4] != b"PE\0\0" {
        return None;
    }
    let n_sections = u16::from_le_bytes([coff[6], coff[7]]) as u64;
    let opt_size = u16::from_le_bytes([coff[20], coff[21]]) as u64;
    let opt_start = pe_off.checked_add(24)?;

    // The optional-header magic decides where NumberOfRvaAndSizes and the data
    // directories sit. Anything else is not an image we will guess about.
    let mut magic = [0u8; 2];
    read_at(&mut f, opt_start, &mut magic)?;
    let (n_rva_off, dirs_off) = match u16::from_le_bytes(magic) {
        0x10B => (92u64, 96u64),  // PE32
        0x20B => (108u64, 112u64), // PE32+
        _ => return None,
    };

    // Directory index 14 is the CLR runtime header. A file declaring fewer than
    // 15 directories has no such entry, and reading it anyway would land in the
    // section table and hand back a garbage RVA that might even resolve.
    let mut n_rva = [0u8; 4];
    read_at(&mut f, opt_start.checked_add(n_rva_off)?, &mut n_rva)?;
    if u32::from_le_bytes(n_rva) <= 14 {
        return None;
    }

    let mut dir = [0u8; 8]; // RVA(4) + Size(4)
    read_at(&mut f, opt_start.checked_add(dirs_off)?.checked_add(14 * 8)?, &mut dir)?;
    let rva = u32::from_le_bytes([dir[0], dir[1], dir[2], dir[3]]) as u64;
    if rva == 0 {
        return None; // no CLI header — an ordinary native binary
    }

    let at = rva_to_file_offset(&mut f, opt_start.checked_add(opt_size)?, n_sections, rva)?;

    // COR20: cb(4) MajorRuntimeVersion(2) MinorRuntimeVersion(2)
    //        MetaData RVA(4)+Size(4) Flags(4)  -> Flags is at +16.
    let mut flags = [0u8; 4];
    read_at(&mut f, at.checked_add(16)?, &mut flags)?;
    Some(u32::from_le_bytes(flags))
}

/// Map a PE on disk to the binary-selection decision.
///
/// `Machine` ALONE DOES NOT ANSWER THIS for .NET, which is this product's whole
/// target population. Every managed assembly is emitted as a PE32 with
/// `Machine == 0x014C`; an AnyCPU one (no `32BITREQUIRED`, no `32BITPREFERRED`)
/// is loaded by the 64-bit CLR as a **64-bit process**. Classifying it X86 picks
/// `procdump.exe`, and 32-bit ProcDump cannot attach to a 64-bit target at all —
/// no dump, not a truncated one. So an I386 machine word means "consult the
/// COR20 header", not "X86".
///
/// ILONLY is deliberately NOT required. Adding it could only move an image from
/// X64 to X86, which is the catastrophic direction; and it buys nothing, because
/// mixed-mode (non-ILONLY) x86 images get `32BITREQUIRED` from the linker and
/// already land on X86 here.
///
/// X64 here means "the CLR runs this 64-bit ON A 64-BIT OS". The 32-bit-OS case
/// is `select_binary`'s job (it short-circuits on `os_is_64`), not this
/// function's — do not add an env lookup to keep it pure.
///
/// NOTE: ARM64 and AMD64 both map to X64. That is correct for choosing a
/// ProcDump binary, but it means the two are indistinguishable here — do not
/// assert anything stronger.
pub fn bitness_from_pe(path: &Path) -> Bitness {
    match pe_machine(path) {
        Some(IMAGE_FILE_MACHINE_I386) => match cor20_flags(path) {
            // Managed and neither flag set: AnyCPU -> runs 64-bit.
            Some(f)
                if f & COMIMAGE_FLAGS_32BITREQUIRED == 0
                    && f & COMIMAGE_FLAGS_32BITPREFERRED == 0 =>
            {
                Bitness::X64
            }
            // Managed-but-pinned-32, unmanaged, or unwalkable: genuinely x86.
            _ => Bitness::X86,
        },
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

const SERVICES_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Services";

/// True unless we are on a 32-bit OS.
///
/// The single source of truth, and now actually the only one: Task 5 replaced
/// the monitor's inline copy (a case-SENSITIVE `!= "x86"`) with a call to this,
/// and Task 6 replaced the GUI label's hardcoded `true`. `monitor.rs` and
/// `gui/page_monitor.rs::bitness_text` are the only callers — keep it that way,
/// a second inline copy is how the label started lying.
pub fn os_is_64() -> bool {
    os_is_64_from(
        std::env::var("PROCESSOR_ARCHITECTURE").ok().as_deref(),
        std::env::var("PROCESSOR_ARCHITEW6432").ok().as_deref(),
    )
}

/// Pure core of `os_is_64`, split out so both branches are testable without
/// mutating process-global env vars under parallel test threads.
///
/// `PROCESSOR_ARCHITEW6432` is only set inside a 32-bit process on 64-bit
/// Windows (WOW64), where `PROCESSOR_ARCHITECTURE` reads "x86" — so it is the
/// tie-breaker, not a second opinion. Unset/unreadable falls back to 64-bit:
/// a 32-bit OS in 2026 is the rarer wrong answer.
fn os_is_64_from(arch: Option<&str>, arch_w6432: Option<&str>) -> bool {
    // eq_ignore_ascii_case, not `!= "x86"`: casing of this value is not
    // contractual and getting it wrong picks the wrong ProcDump binary.
    arch.map(|a| !a.eq_ignore_ascii_case("x86")).unwrap_or(true) || arch_w6432.is_some()
}

/// Read a service's ImagePath from the registry.
///
/// Uses `collect::run_tool`, which already sets CREATE_NO_WINDOW — a bare
/// Command spawn from the GUI would stall the message pump.
#[cfg(windows)]
pub fn service_image_path(service: &str) -> Option<PathBuf> {
    let key = format!(r"{SERVICES_KEY}\{service}");
    let out = crate::collect::run_tool("reg.exe", &["query", &key, "/v", "ImagePath"]).ok()?;
    if !out.status.success() {
        return None;
    }
    image_path_from_reg_output(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(windows))]
pub fn service_image_path(_service: &str) -> Option<PathBuf> {
    None
}

/// Pick the ImagePath value out of `reg.exe query` output and turn it into a
/// path we are willing to hand onward. Split from `service_image_path` (and
/// left ungated) so every rule below is unit-testable from a string literal
/// instead of needing a real service key.
///
/// Returns None for svchost-hosted services: the shared host's PE says nothing
/// about the bitness of the service DLL it loads, so resolution should fall
/// through to runtime detection rather than confidently answer "64-bit".
fn image_path_from_reg_output(text: &str) -> Option<PathBuf> {
    // reg.exe line: "    ImagePath    REG_EXPAND_SZ    C:\path\to.exe -args"
    //
    // ponytail: shelling to reg.exe and scraping its text assumes the en-US
    // token layout — the same documented assumption parse_sc_output already
    // makes for sc.exe in services.rs (C-CURE deployments are en-US). Ceiling:
    // a localized host where the value name or REG_* tokens are translated
    // silently yields None, degrading to runtime detection. Upgrade path:
    // RegGetValueW via the windows crate, which returns the value directly and
    // is locale-proof — worth it only if a non-en-US deployment shows up.
    let raw = text.lines().find_map(|l| {
        let rest = l.trim_start().strip_prefix("ImagePath")?.trim_start();
        let mut parts = rest.splitn(2, char::is_whitespace);
        // The REG_* type token must be where the type belongs. This does two
        // jobs: it rejects a line with no type token (whose first data word
        // would otherwise be silently eaten as the type), and it rejects a
        // differently-named value such as ImagePathBackup, whose name remainder
        // ("Backup") lands in the type slot. A separate "name must be followed
        // by whitespace" check would be strictly redundant with it.
        parts.next().filter(|t| t.starts_with("REG_"))?;
        Some(parts.next()?.trim().to_string())
    })?;

    let path = parse_image_path(&raw)?;
    if path
        .file_name()
        .map(|f| f.to_string_lossy().eq_ignore_ascii_case("svchost.exe"))
        .unwrap_or(false)
    {
        return None; // shared host — fall through to runtime detection
    }
    // parse_image_path is pure and can hand back a non-path: a driver-style
    // ImagePath (system32\DRIVERS\foo.sys) has no ".exe" token so the whole
    // string comes back arguments and all, and relative paths come back
    // relative. This is the trust boundary — every caller of
    // service_image_path routes through here, so the check lives here.
    if !path.exists() {
        return None;
    }
    Some(path)
}

/// Best-effort file path for the configured target.
pub fn resolve_target_path(cfg: &Config) -> Option<PathBuf> {
    match cfg.target_type {
        TargetType::Service => service_image_path(&cfg.target_name),
        TargetType::Process => {
            let p = PathBuf::from(cfg.target_path.trim());
            if !cfg.target_path.trim().is_empty() && p.exists() {
                return Some(p);
            }
            running_process_path(&cfg.target_name)
        }
    }
}

/// Point `cfg` at a target, dropping any cached `target_path` that belonged to
/// the PREVIOUS one.
///
/// CHEAP AND PURE: no filesystem, no registry, no subprocess. That is a
/// requirement, not an accident — the Monitor page calls this from
/// `write_fields`, which runs on every preview refresh, i.e. on every
/// keystroke in seven text boxes. Do not move `capture_target_path`'s work in
/// here (see its doc comment).
///
/// This OWNS the name/type assignment on purpose: the clear rule needs the
/// PREVIOUS name/type, so nothing may assign them first. Without the clear,
/// `resolve_target_path`'s Process arm — which trusts `cfg.target_path` before
/// it ever looks at the name — hands back the OLD target's exe on a target
/// switch, and the capture writes it straight back. `.exists()` cannot catch
/// that: the previous install is usually still on disk.
///
/// Names are compared case-insensitively: Windows process and service names
/// are, and a casing-only difference is the same target.
pub fn set_target(cfg: &mut Config, name: &str, ttype: TargetType) {
    if !cfg.target_name.eq_ignore_ascii_case(name) || cfg.target_type != ttype {
        cfg.target_path.clear();
    }
    cfg.target_name = name.to_string();
    cfg.target_type = ttype;
}

/// Refresh `cfg.target_path` from the live system so bitness survives the
/// target not running.
///
/// EXPENSIVE — for a Service target this shells to `reg.exe`. Call ONLY from
/// real-persist paths (`MonitorPage::save`), never from the preview path.
/// Split out of `set_target` for exactly that reason: fused, it cost one
/// `reg.exe` spawn per typed character on the Monitor page. Nothing reads
/// `target_path` to build the preview (`procdump::build_args` never touches
/// it), so the preview clone does not need this.
///
/// Must run AFTER `set_target` has cleared any stale path, which the
/// `save() -> write_fields() -> set_target()` order guarantees.
///
/// `if let Some` is load-bearing: a failed resolve must KEEP whatever is
/// there. A target that merely stopped running must not lose a good path
/// (Service targets especially — their resolve can fail transiently).
pub fn capture_target_path(cfg: &mut Config) {
    if let Some(p) = resolve_target_path(cfg) {
        cfg.target_path = p.to_string_lossy().to_string();
    }
}

/// Full image path of a running process, found by exe name.
///
/// ponytail: `list_process_names()` dedupes by name, so if two running
/// processes share an exe name whichever Toolhelp returns first wins. Same-
/// named processes are overwhelmingly the same image; upgrade to a PID-based
/// picker only if that assumption breaks in the field.
#[cfg(windows)]
fn running_process_path(name: &str) -> Option<PathBuf> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let want = name.trim_end_matches(".exe").trim_end_matches(".EXE").to_ascii_lowercase();
    if want.is_empty() {
        return None;
    }

    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut pid = 0u32;
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let n = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)],
                );
                if n.trim_end_matches(".exe").eq_ignore_ascii_case(&want) {
                    pid = entry.th32ProcessID;
                    break;
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        if pid == 0 {
            return None;
        }

        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        // ponytail: fixed MAX_PATH buffer. Ceiling: an image path longer than
        // 260 chars fails the call rather than truncating, so we return None
        // and `resolve` degrades to runtime detection — wrong-answer-free, just
        // less capable. Upgrade path: retry on ERROR_INSUFFICIENT_BUFFER with a
        // 32767 buffer, if a long-path install ever appears.
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(h);
        if !ok || len == 0 {
            return None;
        }
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
    }
}

#[cfg(not(windows))]
fn running_process_path(_name: &str) -> Option<PathBuf> {
    None
}

#[cfg(not(windows))]
fn detect(_process_name: &str) -> Bitness {
    Bitness::Unknown
}

/// Ordered bitness resolution. The returned &str names the source, for logs
/// and the GUI label.
///
///   1. PE header from the resolved path — correct even if the target has
///      never run, which is what makes `-w` work. "PE header" is shorthand:
///      `bitness_from_pe` also reads the COR20 header, because for a managed
///      PE32 the `Machine` word alone would misclassify every AnyCPU assembly
///      as X86. Step 2 CANNOT rescue that — an I386 machine word never yields
///      Unknown, so this step always answers and always wins.
///   2. Runtime detection — covers a RUNNING PROCESS target whose PE we could
///      not read (no path resolved, or the file is unreadable/not a PE).
///      Deliberately NOT tried for Service targets: `detect` looks up a
///      process name, and a service name is not one (Spooler -> spoolsv.exe),
///      so it would only ever burn a Toolhelp snapshot to return Unknown.
///   3. Unknown — caller warns loudly and falls back to procdump64.exe.
pub fn resolve(cfg: &Config) -> (Bitness, &'static str) {
    if let Some(p) = resolve_target_path(cfg) {
        let b = bitness_from_pe(&p);
        if b != Bitness::Unknown {
            return (b, "PE header");
        }
    }
    if cfg.target_type == TargetType::Process {
        let b = detect(&cfg.target_name);
        if b != Bitness::Unknown {
            return (b, "running process");
        }
    }
    (Bitness::Unknown, "unresolved")
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

        // ...and an unmanaged AMD64 system binary, so the AMD64 arm is pinned
        // on a file the COR20 walk is never even consulted for.
        let p = std::path::PathBuf::from(r"C:\Windows\System32\cmd.exe");
        if !p.exists() {
            eprintln!("skipping: {} not present on this host", p.display());
            return;
        }
        assert_eq!(pe_machine(&p), Some(0x8664));
        assert_eq!(bitness_from_pe(&p), Bitness::X64);
    }

    // ------------------------------------------------- AnyCPU / COR20 (C1) --

    #[test]
    fn anycpu_managed_pe32_resolves_as_64bit() {
        // THE regression this fix exists for. AddInProcess.exe is a real
        // AnyCPU assembly: machine=0x014C, ILONLY, no 32BITREQUIRED, no
        // 32BITPREFERRED — the 64-bit CLR runs it as a 64-bit process, so
        // procdump.exe could not attach to it at all.
        //
        // Skip (do not fail) if absent: it ships with the .NET Framework, which
        // is not guaranteed on an arbitrary build host.
        let p = std::path::PathBuf::from(
            r"C:\Windows\Microsoft.NET\Framework64\v4.0.30319\AddInProcess.exe",
        );
        if !p.exists() {
            eprintln!("skipping: {} not present on this host", p.display());
            return;
        }
        assert_eq!(pe_machine(&p), Some(0x014C), "precondition: it IS a PE32 I386 image");
        let flags = cor20_flags(&p).expect("precondition: it IS managed");
        assert_eq!(flags & COMIMAGE_FLAGS_32BITREQUIRED, 0, "precondition: not 32BITREQUIRED");
        assert_eq!(flags & COMIMAGE_FLAGS_32BITPREFERRED, 0, "precondition: not 32BITPREFERRED");
        assert_eq!(
            bitness_from_pe(&p),
            Bitness::X64,
            "AnyCPU managed PE32 must not be classified X86"
        );
    }

    /// A minimal but structurally valid PE: DOS header, COFF header, an
    /// optional header with 16 data directories, one section, and a COR20
    /// header at file offset 0x400 (RVA 0x1000).
    ///
    /// Synthetic rather than real because the flag combinations that
    /// distinguish the predicate's two terms DO NOT OCCUR in shipped binaries:
    /// a real prefer-32 assembly sets 32BITREQUIRED *and* 32BITPREFERRED
    /// together, so no real file can kill the 32BITPREFERRED term on its own.
    fn synth_pe(machine: u16, magic: u16, n_rva: u32, clr_rva: u32, flags: u32) -> Vec<u8> {
        let (n_rva_off, dirs_off) = if magic == 0x20B { (108usize, 112usize) } else { (92, 96) };
        let opt_size = dirs_off + 16 * 8;
        let opt = 0x58usize; // 0x40 PE sig + 24-byte COFF header

        let mut v = vec![0u8; 0x600];
        v[0..2].copy_from_slice(b"MZ");
        v[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        v[0x40..0x44].copy_from_slice(b"PE\0\0");
        v[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
        v[0x46..0x48].copy_from_slice(&1u16.to_le_bytes()); // NumberOfSections
        v[0x54..0x56].copy_from_slice(&(opt_size as u16).to_le_bytes());

        v[opt..opt + 2].copy_from_slice(&magic.to_le_bytes());
        v[opt + n_rva_off..opt + n_rva_off + 4].copy_from_slice(&n_rva.to_le_bytes());
        let d14 = opt + dirs_off + 14 * 8;
        v[d14..d14 + 4].copy_from_slice(&clr_rva.to_le_bytes());
        v[d14 + 4..d14 + 8].copy_from_slice(&72u32.to_le_bytes()); // Size

        let sec = opt + opt_size; // section table
        v[sec..sec + 5].copy_from_slice(b".text");
        v[sec + 12..sec + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // VirtualAddress
        v[sec + 16..sec + 20].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfRawData
        v[sec + 20..sec + 24].copy_from_slice(&0x400u32.to_le_bytes()); // PointerToRawData

        v[0x400..0x404].copy_from_slice(&72u32.to_le_bytes()); // COR20 cb
        v[0x410..0x414].copy_from_slice(&flags.to_le_bytes()); // COR20 Flags (+16)
        v
    }

    /// Write `bytes` to a uniquely named temp file and hand back the path.
    fn tmp_pe(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("pdm_pe_{tag}_{n}.bin"));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    const ILONLY: u32 = 0x1;

    #[test]
    fn managed_pe32_flags_decide_the_anycpu_answer() {
        // The truth table the predicate encodes. Each row kills a mutant:
        //  * row 1 vs row 2 kills `!32BITREQUIRED`
        //  * row 1 vs row 3 kills `!32BITPREFERRED` — and ONLY a synthetic file
        //    can, because real prefer-32 images set both flags (row 4).
        for (flags, want, why) in [
            (ILONLY, Bitness::X64, "AnyCPU"),
            (ILONLY | COMIMAGE_FLAGS_32BITREQUIRED, Bitness::X86, "x86-only"),
            (ILONLY | COMIMAGE_FLAGS_32BITPREFERRED, Bitness::X86, "prefer-32 alone"),
            (
                ILONLY | COMIMAGE_FLAGS_32BITREQUIRED | COMIMAGE_FLAGS_32BITPREFERRED,
                Bitness::X86,
                "prefer-32 as really shipped",
            ),
            // ILONLY clear with neither pin. This row asserts THE CURRENT
            // PREDICATE, not OS semantics: a managed PE32 without ILONLY holds
            // native code and would really run 32-bit, so X64 is only "right"
            // in the sense that we deliberately do not gate on ILONLY (see
            // bitness_from_pe). The shape does not ship — mixed-mode images get
            // 32BITREQUIRED from the linker (row 2). Kept so the no-ILONLY-gate
            // decision is mechanically pinned; if you ever DO gate on it,
            // change this row rather than assuming the test found a bug.
            (0, Bitness::X64, "no ILONLY, no pin (predicate, not OS semantics)"),
        ] {
            let p = tmp_pe("flags", &synth_pe(0x014C, 0x10B, 16, 0x1000, flags));
            let got = bitness_from_pe(&p);
            let _ = std::fs::remove_file(&p);
            assert_eq!(got, want, "flags={flags:#x} ({why})");
        }
    }

    #[test]
    fn unmanaged_i386_stays_x86() {
        // CLR directory RVA == 0. Kills a mutant that answers X64 whenever the
        // machine word is I386 — i.e. the "just flip I386 to X64" non-fix.
        let p = tmp_pe("native", &synth_pe(0x014C, 0x10B, 16, 0, ILONLY));
        let got = bitness_from_pe(&p);
        let flags = cor20_flags(&p);
        let _ = std::fs::remove_file(&p);
        assert_eq!(flags, None, "RVA 0 must read as unmanaged");
        assert_eq!(got, Bitness::X86);
    }

    #[test]
    fn amd64_never_consults_the_cor20_header() {
        // A managed AMD64 image (Machine 0x8664 + PE32+) with 32BITREQUIRED
        // set. The flags say x86; the machine word wins, as it must.
        let p = tmp_pe(
            "amd64mgd",
            &synth_pe(0x8664, 0x20B, 16, 0x1000, ILONLY | COMIMAGE_FLAGS_32BITREQUIRED),
        );
        let got = bitness_from_pe(&p);
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, Bitness::X64);
    }

    #[test]
    fn cor20_walk_rejects_malformed_images_without_panicking() {
        // Every failure mode of the walk, all of which must degrade to the
        // machine word's X86 rather than abort the process (panic = "abort").
        let cases: Vec<(&str, Vec<u8>)> = vec![
            // Fewer than 15 data directories: index 14 does not exist, even
            // though the bytes there look like a usable RVA.
            ("few_dirs", synth_pe(0x014C, 0x10B, 14, 0x1000, ILONLY)),
            // RVA that no section covers.
            ("bad_rva", synth_pe(0x014C, 0x10B, 16, 0xFFFF_F000, ILONLY)),
            // RVA whose section maps it past the end of the file.
            ("rva_past_eof", synth_pe(0x014C, 0x10B, 16, 0x11FF, ILONLY)),
            // Optional-header magic that is neither PE32 nor PE32+.
            ("bad_magic", synth_pe(0x014C, 0xDEAD, 16, 0x1000, ILONLY)),
        ];
        for (tag, bytes) in cases {
            let p = tmp_pe(tag, &bytes);
            let got = bitness_from_pe(&p);
            let flags = cor20_flags(&p);
            let _ = std::fs::remove_file(&p);
            assert_eq!(flags, None, "{tag}: walk should have refused");
            assert_eq!(got, Bitness::X86, "{tag}");
        }
    }

    #[test]
    fn cor20_walk_survives_truncation_at_every_length() {
        // Truncate a valid managed PE32 at every 16-byte boundary. Any read the
        // walk performs can land past EOF; none may panic. Answers are only
        // ever X86 (walk refused) or X64 (walk completed) — never a crash.
        let full = synth_pe(0x014C, 0x10B, 16, 0x1000, ILONLY);
        for len in (0..full.len()).step_by(16) {
            let p = tmp_pe("trunc", &full[..len]);
            let got = bitness_from_pe(&p);
            let _ = std::fs::remove_file(&p);
            assert!(
                matches!(got, Bitness::X86 | Bitness::X64 | Bitness::Unknown),
                "len={len} produced {got:?}"
            );
        }
    }

    #[test]
    fn the_section_walk_is_capped_at_the_pe_spec_limit() {
        // NumberOfSections is an attacker-controlled u16; without the cap a
        // malformed file costs up to 65535 seek+read pairs inside the monitor
        // loop. A short file bounds that by EOF anyway, so timing a small
        // fixture proves NOTHING — the cap is only observable when the file is
        // big enough to hold more than 96 section headers.
        //
        // So: put the ONLY mapping section at index 200, past the cap but well
        // inside the file. Capped, the walk refuses (X86); delete `.min(96)`
        // and it finds the section and answers X64. That is the mutant kill.
        let mut v = synth_pe(0x014C, 0x10B, 16, 0x1000, ILONLY);
        let sec = 0x58 + 96 + 16 * 8; // opt_start + SizeOfOptionalHeader
        v.resize(0x3200, 0);
        v[0x46..0x48].copy_from_slice(&300u16.to_le_bytes()); // NumberOfSections
        v[sec + 12..sec + 24].fill(0); // section 0 now maps nothing
        let s200 = sec + 200 * 40;
        v[s200 + 12..s200 + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // VirtualAddress
        v[s200 + 16..s200 + 20].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfRawData
        v[s200 + 20..s200 + 24].copy_from_slice(&0x3100u32.to_le_bytes()); // PointerToRawData
        v[0x3110..0x3114].copy_from_slice(&ILONLY.to_le_bytes()); // COR20 Flags (+16)

        let p = tmp_pe("section200", &v);
        let got = bitness_from_pe(&p);
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, Bitness::X86, "the walk looked past section 96");

        // ...and the pathological count itself must still just refuse.
        let mut v = synth_pe(0x014C, 0x10B, 16, 0xFFFF_F000, ILONLY);
        v[0x46..0x48].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let p = tmp_pe("many_sections", &v);
        let got = bitness_from_pe(&p);
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, Bitness::X86);
    }

    #[test]
    fn cor20_flags_refuses_garbage_and_non_pe_files() {
        let junk = tmp_pe("junk", &[0xABu8; 4096]);
        assert_eq!(cor20_flags(&junk), None);
        let _ = std::fs::remove_file(&junk);

        // Valid DOS header, garbage e_lfanew.
        let bad = tmp_pe("badoff", &dos_header(0xFFFF_FFF0));
        assert_eq!(cor20_flags(&bad), None);
        assert_eq!(bitness_from_pe(&bad), Bitness::Unknown);
        let _ = std::fs::remove_file(&bad);

        let missing = std::env::temp_dir().join("pdm_cor20_does_not_exist.exe");
        assert_eq!(cor20_flags(&missing), None);
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

    // ------------------------------------------------------------ task 3 --

    #[test]
    fn os_is_64_is_true_on_this_dev_machine() {
        // The build/test host is x64; this guards a regression in the env parsing.
        assert!(os_is_64());
    }

    #[test]
    fn os_is_64_from_covers_every_arm() {
        // The test above cannot fail on this host (every arm but one returns
        // true). This table is where os_is_64's logic is actually pinned.
        assert!(os_is_64_from(Some("AMD64"), None), "native x64");
        assert!(os_is_64_from(Some("ARM64"), None), "native arm64");
        assert!(!os_is_64_from(Some("x86"), None), "genuine 32-bit OS");
        assert!(!os_is_64_from(Some("X86"), None), "casing must not flip the answer");
        assert!(os_is_64_from(Some("x86"), Some("AMD64")), "32-bit process under WOW64");
        assert!(os_is_64_from(None, None), "unreadable env -> assume 64-bit");
    }

    /// Shape of a real `reg.exe query ... /v ImagePath` response.
    fn reg_output(line: &str) -> String {
        format!(
            "\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Services\\Test\r\n    {line}\r\n\r\n"
        )
    }

    #[test]
    fn reg_output_yields_the_image_path() {
        // Uses the test exe (guaranteed to exist) so the .exists() gate passes.
        let me = std::env::current_exe().unwrap();
        let out = reg_output(&format!("ImagePath    REG_EXPAND_SZ    {}", me.display()));
        assert_eq!(image_path_from_reg_output(&out), Some(me));
    }

    #[test]
    fn reg_output_rejects_a_similarly_named_value() {
        // A bare starts_with("ImagePath") reads ImagePathBackup's data as ours.
        // Rejected because "Backup" lands in the REG_* type slot.
        // Honesty note from the mutation run: this test does NOT uniquely kill
        // the "REG_ check removed" mutant — with that check gone the value
        // parses as the literal string "REG_SZ    C:\...", which .exists()
        // then rejects. It is defence in depth, kept for the intent it pins.
        let me = std::env::current_exe().unwrap();
        let out = reg_output(&format!("ImagePathBackup    REG_SZ    {}", me.display()));
        assert_eq!(image_path_from_reg_output(&out), None);
    }

    #[test]
    fn reg_output_rejects_a_wrong_type_token() {
        // Kills the starts_with("REG_") check: without it NOTAREGTYPE is eaten
        // as the type and the REAL path behind it is returned as the value.
        let me = std::env::current_exe().unwrap();
        let out = reg_output(&format!("ImagePath    NOTAREGTYPE    {}", me.display()));
        assert_eq!(image_path_from_reg_output(&out), None);

        // The genuinely-MISSING-token case D4 cites: with no type token the
        // path itself lands in the type slot and the arguments become the
        // "value". Honesty note, same as the ImagePathBackup test: this half
        // does not uniquely kill the mutant — drop the REG_ check and it still
        // returns None, because the leftover "-args" fails .exists(). Kept
        // because it pins the documented failure mode, not because it guards.
        let out = reg_output(r"ImagePath    C:\x.exe -args");
        assert_eq!(image_path_from_reg_output(&out), None);
    }

    #[test]
    fn reg_output_rejects_a_driver_style_image_path() {
        // Task 2 carry-forward: no ".exe" token, so parse_image_path returns
        // the whole string (arguments included) and the path is relative.
        // Only the .exists() gate stops this reaching bitness_from_pe.
        let out = reg_output(r"ImagePath    REG_EXPAND_SZ    system32\DRIVERS\pdm_no_such.sys");
        assert_eq!(image_path_from_reg_output(&out), None);
    }

    #[test]
    fn reg_output_rejects_a_nonexistent_exe() {
        let out = reg_output(r"ImagePath    REG_EXPAND_SZ    C:\pdm_no_such_dir\nope.exe");
        assert_eq!(image_path_from_reg_output(&out), None);
    }

    #[test]
    fn reg_output_rejects_svchost_hosted_services() {
        // A shared host's PE says nothing about the hosted service, so this
        // must be None (falls through to runtime detection), NOT the x64
        // answer svchost.exe's own header would give.
        let out = reg_output(r"ImagePath    REG_EXPAND_SZ    %SystemRoot%\system32\svchost.exe -k netsvcs -p");
        assert_eq!(image_path_from_reg_output(&out), None);
        // ...and svchost.exe really does exist, so .exists() is not what
        // rejected it — the svchost rule is.
        let svchost = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap())
            .join(r"system32\svchost.exe");
        assert!(svchost.exists(), "precondition: {} must exist", svchost.display());
    }

    #[test]
    fn reg_output_with_no_image_path_value_is_none() {
        assert_eq!(image_path_from_reg_output(""), None);
        assert_eq!(image_path_from_reg_output(&reg_output("Start    REG_DWORD    0x2")), None);
    }

    #[test]
    fn service_image_path_resolves_a_real_windows_service() {
        // Spooler exists on every Windows host and is a standalone exe, not a
        // svchost-hosted service.
        match service_image_path("Spooler") {
            Some(p) => {
                let s = p.to_string_lossy().to_ascii_lowercase();
                assert!(s.ends_with("spoolsv.exe"), "unexpected ImagePath: {s}");
                assert!(p.exists(), "resolved path does not exist: {s}");
            }
            None => eprintln!("skipping: Spooler service not present"),
        }
    }

    #[test]
    fn service_image_path_returns_none_for_unknown_service() {
        assert_eq!(service_image_path("PdmDefinitelyNotAService"), None);
    }

    #[test]
    fn service_image_path_returns_none_for_a_real_svchost_service() {
        // Proves service_image_path is actually wired to the svchost rule, not
        // just that the rule exists in the helper. Skips rather than fails if
        // this host does not host Winmgmt in svchost.
        let out = crate::collect::run_tool(
            "reg.exe",
            &["query", r"HKLM\SYSTEM\CurrentControlSet\Services\Winmgmt", "/v", "ImagePath"],
        );
        let hosted = match &out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
                .to_ascii_lowercase()
                .contains("svchost.exe"),
            _ => false,
        };
        if !hosted {
            eprintln!("skipping: Winmgmt is not svchost-hosted on this host");
            return;
        }
        assert_eq!(service_image_path("Winmgmt"), None);
    }

    /// The running test binary's exe name — a process guaranteed to be running
    /// while these tests execute, so `detect` can always resolve it.
    fn my_process_name() -> String {
        std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn resolve_target_path_prefers_a_valid_configured_path() {
        let me = std::env::current_exe().unwrap();
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = "PdmDefinitelyNotRunning.exe".into();
        c.target_path = me.to_string_lossy().to_string();
        assert_eq!(resolve_target_path(&c), Some(me));
    }

    #[test]
    fn resolve_target_path_ignores_a_stale_configured_path() {
        // Kills the `p.exists()` guard in the Process branch: a config left
        // over from an uninstalled build must fall through to the live lookup,
        // not hand a dead path to bitness_from_pe.
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = my_process_name();
        c.target_path = r"C:\pdm_no_such_dir\stale.exe".into();
        let p = resolve_target_path(&c).expect("should fall back to the running process");
        // Case-insensitive: QueryFullProcessImageNameW and current_exe() can
        // disagree on drive-letter/component casing.
        assert!(
            p.to_string_lossy()
                .eq_ignore_ascii_case(&std::env::current_exe().unwrap().to_string_lossy()),
            "got {}",
            p.display()
        );
    }

    #[test]
    fn resolve_target_path_is_none_when_nothing_resolves() {
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = "PdmDefinitelyNotRunning.exe".into();
        assert_eq!(resolve_target_path(&c), None);
    }

    #[test]
    fn resolve_uses_pe_for_a_service_target() {
        let mut c = Config::default();
        c.target_type = TargetType::Service;
        c.target_name = "Spooler".into();
        if service_image_path("Spooler").is_none() {
            eprintln!("skipping: Spooler not present");
            return;
        }
        let (b, source) = resolve(&c);
        // spoolsv.exe is x64 on an x64 host.
        assert_eq!(b, Bitness::X64);
        assert_eq!(source, "PE header");
    }

    #[test]
    fn resolve_prefers_the_pe_over_runtime_detect() {
        // The two sources DISAGREE here: target_path is a 32-bit PE while
        // target_name is this very test process (running, 64-bit). Correct
        // ordering answers X86/"PE header"; swap the two blocks in `resolve`
        // and this returns X64/"running process".
        let wow = std::path::PathBuf::from(r"C:\Windows\SysWOW64\cmd.exe");
        if !wow.exists() {
            eprintln!("skipping: {} not present on this host", wow.display());
            return;
        }
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = my_process_name();
        c.target_path = wow.to_string_lossy().to_string();
        assert_eq!(detect(&c.target_name), Bitness::X64, "precondition: we are a running x64 process");
        assert_eq!(resolve(&c), (Bitness::X86, "PE header"));
    }

    #[test]
    fn resolve_falls_back_to_runtime_detect_when_the_pe_is_unreadable() {
        // Proves step 2 is reachable and that the `b != Unknown` guard on step
        // 1 matters: the configured path exists but is not a PE, so the PE
        // read yields Unknown and runtime detection must supply the answer.
        let junk = std::env::temp_dir().join("pdm_resolve_notpe.txt");
        std::fs::write(&junk, b"definitely not a PE").unwrap();
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = my_process_name();
        c.target_path = junk.to_string_lossy().to_string();
        let got = resolve(&c);
        let _ = std::fs::remove_file(&junk);
        assert_eq!(got, (Bitness::X64, "running process"));
    }

    #[test]
    fn resolve_does_not_runtime_detect_a_service_target() {
        // Kills the `target_type == Process` gate on step 2. The target name
        // is a RUNNING process, but it is declared as a service — the registry
        // lookup finds no such key, and detect() must not be consulted.
        let mut c = Config::default();
        c.target_type = TargetType::Service;
        c.target_name = my_process_name();
        assert_eq!(detect(&c.target_name), Bitness::X64, "precondition: detect would answer");
        assert_eq!(resolve(&c), (Bitness::Unknown, "unresolved"));
    }

    #[test]
    fn resolve_of_a_default_config_is_unresolved() {
        // CORRECTED (task 6): an earlier version of this comment claimed the
        // GUI's 3s poll timer calls resolve(). It does not — the timer reaches
        // only refresh_status, and gui/page_monitor.rs::bitness_text
        // short-circuits on an empty target BEFORE resolve, precisely so an
        // empty name cannot spawn reg.exe. The GUI never reaches this input.
        //
        // CORRECTED AGAIN (final review, I3): the monitor no longer reaches
        // this input either. cli.rs's `monitor` verb still does no target
        // validation and Config::load still falls back to Config::default() on
        // a missing or unparseable file — but monitor::run now rejects an empty
        // target_name and exits BEFORE the loop, so the "resolve(&default())
        // once per cycle, forever" path this comment used to describe is gone.
        //
        // The assertion is therefore a pure-function guard now, not a
        // production-path one: it pins what `resolve` itself does with an empty
        // name, so a future caller that reaches it cannot be surprised.
        //
        // What the assertion pins: running_process_path guards on is_empty;
        // detect() does not, and matches on `"" == ""`. Nothing else stops an
        // empty name latching onto a nameless process.
        assert_eq!(resolve(&Config::default()), (Bitness::Unknown, "unresolved"));
    }

    // ------------------------------------------------------------ task 4 --

    #[test]
    fn set_target_does_not_resolve_or_capture() {
        // set_target runs on EVERY preview refresh -- which is every keystroke
        // in the Monitor page's seven option text boxes. It must stay free of
        // filesystem/registry work; for a Service target a resolve here is a
        // reg.exe spawn per typed character.
        //
        // The target used here IS running, so resolve_target_path would
        // happily answer. Fuse the capture back into set_target and this goes
        // red -- which is the whole point of the split.
        let mut c = Config::default();
        set_target(&mut c, &my_process_name(), TargetType::Process);
        assert_eq!(c.target_name, my_process_name());
        assert_eq!(c.target_type, TargetType::Process);
        assert_eq!(c.target_path, "", "set_target must not resolve");
        // ...and prove the emptiness above is a real abstention, not the
        // target being unresolvable.
        capture_target_path(&mut c);
        assert!(!c.target_path.is_empty(), "precondition: this target DOES resolve");
    }

    #[test]
    fn capture_target_path_captures_the_running_process_path() {
        // The happy path, and the only test that pins the `cfg.target_path = p`
        // write itself. Delete that line and every other test here still
        // passes (they all assert on clearing/preserving).
        let mut c = Config::default();
        c.target_name = my_process_name();
        assert_eq!(c.target_path, "", "precondition: nothing cached");
        capture_target_path(&mut c);
        assert!(!c.target_path.is_empty(), "capture wrote nothing");
        assert!(
            c.target_path
                .eq_ignore_ascii_case(&std::env::current_exe().unwrap().to_string_lossy()),
            "got {}",
            c.target_path
        );
    }

    #[test]
    fn set_target_then_capture_does_not_re_bless_the_previous_targets_path() {
        // The composed sequence a real persist performs
        // (save -> write_fields -> set_target, then save -> capture), and the
        // regression guard for the defect in the brief's snippet. The old
        // target's exe is still installed, so .exists() is happy and
        // resolve_target_path's Process arm would hand the OLD path straight
        // back -- cementing a wrong PE forever. Drop the clear from set_target
        // and this goes red even though each half looks fine alone.
        let me = std::env::current_exe().unwrap();
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = "PdmPreviousTarget.exe".into();
        c.target_path = me.to_string_lossy().to_string();
        assert!(me.exists(), "precondition: the old exe is still on disk");

        set_target(&mut c, "PdmDefinitelyNotRunning.exe", TargetType::Process);
        capture_target_path(&mut c);
        assert_eq!(c.target_path, "", "re-blessed the previous target's path");
    }

    #[test]
    fn set_target_clears_a_stale_path_when_the_name_changes() {
        // The clear rule in isolation (the composed consequence is pinned by
        // set_target_then_capture_does_not_re_bless_the_previous_targets_path).
        // Drop the name comparison and target_path stays the previous exe.
        let me = std::env::current_exe().unwrap();
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = "PdmPreviousTarget.exe".into();
        c.target_path = me.to_string_lossy().to_string();
        assert!(me.exists(), "precondition: the old exe is still on disk");

        set_target(&mut c, "PdmDefinitelyNotRunning.exe", TargetType::Process);
        assert_eq!(c.target_path, "", "stale path survived a target switch");
        assert_eq!(c.target_name, "PdmDefinitelyNotRunning.exe");
    }

    #[test]
    fn set_target_clears_a_stale_path_when_only_the_type_changes() {
        // Same name, Process -> Service. Kills the `cfg.target_type != ttype`
        // half of the condition: without it nothing clears, the Service arm
        // ignores target_path and returns None, so the process-era path
        // survives on a service target.
        let me = std::env::current_exe().unwrap();
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = my_process_name();
        c.target_path = me.to_string_lossy().to_string();

        set_target(&mut c, &my_process_name(), TargetType::Service);
        assert_eq!(c.target_path, "", "process-era path survived a type switch");
        assert_eq!(c.target_type, TargetType::Service);
    }

    #[test]
    fn capture_target_path_keeps_a_good_path_when_resolve_fails() {
        // Service target that no longer resolves (uninstalled / reg.exe
        // hiccup). The cached path must survive — losing it is how a stopped
        // target loses its bitness. Kills both an unconditional clear and a
        // `= resolve().unwrap_or_default()` shaped write.
        let me = std::env::current_exe().unwrap().to_string_lossy().to_string();
        let mut c = Config::default();
        c.target_type = TargetType::Service;
        c.target_name = "PdmDefinitelyNotAService".into();
        c.target_path = me.clone();
        assert_eq!(
            resolve_target_path(&c),
            None,
            "precondition: this target must not resolve"
        );

        set_target(&mut c, "PdmDefinitelyNotAService", TargetType::Service);
        capture_target_path(&mut c);
        assert_eq!(c.target_path, me, "a failed resolve erased a good path");
    }

    #[test]
    fn set_target_treats_a_casing_only_name_change_as_the_same_target() {
        // Windows service/process names are case-insensitive; `!=` here would
        // throw the cached path away every time the dropdown label's casing
        // differed from what was saved.
        let me = std::env::current_exe().unwrap().to_string_lossy().to_string();
        let mut c = Config::default();
        c.target_type = TargetType::Service;
        c.target_name = "PdmDefinitelyNotAService".into();
        c.target_path = me.clone();

        set_target(&mut c, "PDMDEFINITELYNOTASERVICE", TargetType::Service);
        assert_eq!(c.target_path, me, "casing-only change cleared the path");
        assert_eq!(c.target_name, "PDMDEFINITELYNOTASERVICE", "name still updates");
    }

    #[test]
    fn resolve_falls_back_to_unknown_for_an_unresolvable_target() {
        let mut c = Config::default();
        c.target_type = TargetType::Process;
        c.target_name = "PdmDefinitelyNotRunning.exe".into();
        let (b, source) = resolve(&c);
        assert_eq!(b, Bitness::Unknown);
        assert_eq!(source, "unresolved");
    }
}

# Bitness Resolution + CrossFire Default — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make ProcDumpMonitor launch the ProcDump binary that matches the target's bitness — including when the target is a service or is not yet running — and default the picker to the CrossFire server process.

**Architecture:** Read `IMAGE_FILE_HEADER.Machine` from the target's PE **on disk**, which needs no running process and therefore fixes the `-w` (wait-for-process) case. Service image paths come from `ImagePath` in the registry via `reg.exe`, already the established pattern in `collect/discover.rs`. Runtime `IsWow64Process2` detection stays as a fallback. All new code lands in the existing `rust/src/bitness.rs`; no new files, no new crates.

**Tech Stack:** Rust (stable-msvc), `windows` 0.58 (already present), `std::fs`, `native-windows-gui` 1.0.13. Cargo is NOT on PATH.

## Global Constraints

- Cargo: `%USERPROFILE%\.cargo\bin\cargo.exe`, run from `rust/`. NOT on PATH.
- `cargo test` and any debug build require env `PDM_TEST_MANIFEST=1` (the admin manifest makes the test exe unlaunchable from an unelevated shell). `build.rs` PANICS if that env is set on a `--release` build.
- **No new crate dependencies.** New Windows API surface comes from the already-present `windows` crate or `std`.
- Binary size gate ~2.0MB; currently 1.97MB, so ~30KB headroom. Measure and report the delta; do not assume it fits.
- Release manifest keeps `requireAdministrator`.
- Any `Command` spawn must set `CREATE_NO_WINDOW` or the GUI message pump stalls. Reuse `crate::collect::run_tool`, which already does.
- All GUI coordinates are LOGICAL px. Never multiply by `scale_factor()` outside raw GDI paint paths.
- Do NOT re-merge `write_fields()` into `save()` in `page_monitor.rs` — that split is a data-loss guard.
- Do NOT call `MonitorPage::save()` on a throwaway `Config` clone; use `write_fields()`.
- Config JSON is PascalCase (`#[serde(rename_all = "PascalCase")]`, struct-level `#[serde(default)]`).
- Commit prefixes: `fix(monitor):` / `feat(bitness):` / `gui:` / `docs:` / `verify:`.
- Deliberate shortcuts get a `ponytail:` comment naming the ceiling.

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `rust/src/bitness.rs` | All bitness logic: PE reading, ImagePath parsing, path resolution, ordered resolution, existing `select_binary` + `detect` | Modify (all new functions land here) |
| `rust/src/config.rs` | Add `target_path` field (`TargetPath` on the wire) | Modify |
| `rust/src/monitor.rs:37-48` | Replace one-shot startup selection with cached per-cycle resolution | Modify |
| `rust/src/gui/page_monitor.rs:480-491` | `update_bitness` uses the same `resolve()`; fix hardcoded `os_is_64: true` | Modify |
| `rust/src/gui/page_monitor.rs:349-385, 549+` | CrossFire default selection in `refresh_targets` / `load` | Modify |
| `scripts/gui-e2e.ps1:191-213` | Harden the scroll probe against foreground loss | Modify |

`bitness.rs` is ~260 lines today and will grow to ~450. That is still one clear responsibility (deciding which ProcDump binary to run), so it stays one file.

---

### Task 1: Read the PE machine field from disk

**Files:**
- Modify: `rust/src/bitness.rs`
- Test: `rust/src/bitness.rs` (`#[cfg(test)] mod tests`, same file — the codebase's convention)

**Interfaces:**
- Consumes: existing `pub enum Bitness { Unknown, X86, X64 }`
- Produces: `pub fn pe_machine(path: &Path) -> Option<u16>`, `pub fn bitness_from_pe(path: &Path) -> Bitness`

Background: a PE file stores a 4-byte little-endian offset at `0x3C` (`e_lfanew`) pointing at the PE signature `"PE\0\0"` (`0x00004550`). The `IMAGE_FILE_HEADER` follows immediately; its first field is the 2-byte `Machine`. This was validated against real binaries before this plan was written:

| File | sig | machine |
|---|---|---|
| `C:\Windows\System32\cmd.exe` | `0x4550` | `0x8664` (X64) |
| `C:\Windows\SysWOW64\cmd.exe` | `0x4550` | `0x014C` (X86) |

- [ ] **Step 1: Write the failing tests**

Add inside the existing `mod tests` in `rust/src/bitness.rs`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test bitness::tests::pe_ -- --nocapture
```

Expected: FAIL to compile — `cannot find function 'pe_machine' in this scope`.

- [ ] **Step 3: Implement**

Add near the top of `rust/src/bitness.rs`, after the `use` lines:

```rust
const IMAGE_FILE_MACHINE_I386: u16 = 0x014C;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;

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
```

The existing `classify()` declares the same three constants locally; delete those local declarations and let it use these module-level ones.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test bitness:: -- --nocapture
```

Expected: PASS, including the 6 pre-existing `select_binary` tests.

- [ ] **Step 5: Commit**

```bash
git add rust/src/bitness.rs
git commit -m "feat(bitness): read IMAGE_FILE_HEADER.Machine from the PE on disk

Needs no running process, which is what makes the -w (wait for process)
case resolvable. Guards a garbage e_lfanew against the file length so a
truncated or non-PE file returns None instead of panicking."
```

---

### Task 2: Parse a service ImagePath into a real file path

**Files:**
- Modify: `rust/src/bitness.rs`
- Test: `rust/src/bitness.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `pub fn parse_image_path(raw: &str) -> Option<PathBuf>`

`ImagePath` values are messy in practice: quoted or not, with or without trailing arguments, sometimes prefixed `\??\`, sometimes containing environment variables. A shared-host service resolves to `svchost.exe`, whose PE says nothing about the hosted service.

- [ ] **Step 1: Write the failing tests**

```rust
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
fn image_path_expands_environment_variables() {
    // %SystemRoot% is set on every Windows host.
    let got = parse_image_path(r"%SystemRoot%\system32\services.exe").unwrap();
    let s = got.to_string_lossy().to_ascii_lowercase();
    assert!(!s.contains('%'), "env var was not expanded: {s}");
    assert!(s.ends_with(r"\system32\services.exe"), "unexpected: {s}");
}

#[test]
fn image_path_rejects_empty() {
    assert_eq!(parse_image_path(""), None);
    assert_eq!(parse_image_path("   "), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test bitness::tests::image_path -- --nocapture
```

Expected: FAIL to compile — `cannot find function 'parse_image_path'`.

- [ ] **Step 3: Implement**

```rust
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
        // Unquoted: split after the first ".exe" token. Ambiguous in theory
        // (an unquoted path containing spaces AND arguments), but this is
        // what well-formed service entries look like.
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
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test bitness:: -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/bitness.rs
git commit -m "feat(bitness): parse service ImagePath into a usable file path

Handles quoted paths, trailing arguments, the \\??\\ NT prefix, and
embedded environment variables. Pure and directly unit-testable; the
registry read lands separately."
```

---

### Task 3: Ordered target resolution

**Files:**
- Modify: `rust/src/bitness.rs`
- Test: `rust/src/bitness.rs`

**Interfaces:**
- Consumes: `bitness_from_pe` (Task 1), `parse_image_path` (Task 2), existing `detect`, `crate::collect::run_tool`, `crate::config::{Config, TargetType}`
- Produces:
  - `pub fn os_is_64() -> bool`
  - `pub fn service_image_path(service: &str) -> Option<PathBuf>`
  - `pub fn resolve_target_path(cfg: &Config) -> Option<PathBuf>`
  - `pub fn resolve(cfg: &Config) -> (Bitness, &'static str)`

Note `resolve_target_path` reads `cfg.target_path`, which Task 4 adds. **Do Task 4's config field first if you hit a compile error** — or add the field as part of this task and drop it from Task 4. The plan keeps them separate because the GUI capture logic is independently reviewable.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn os_is_64_is_true_on_this_dev_machine() {
    // The build/test host is x64; this guards a regression in the env parsing.
    assert!(os_is_64());
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
fn resolve_falls_back_to_unknown_for_an_unresolvable_target() {
    let mut c = Config::default();
    c.target_type = TargetType::Process;
    c.target_name = "PdmDefinitelyNotRunning.exe".into();
    let (b, source) = resolve(&c);
    assert_eq!(b, Bitness::Unknown);
    assert_eq!(source, "unresolved");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test bitness::tests::resolve -- --nocapture
```

Expected: FAIL to compile — `cannot find function 'resolve'`.

- [ ] **Step 3: Implement**

```rust
const SERVICES_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Services";

/// True unless we are on a 32-bit OS. Extracted so the GUI and the monitor
/// cannot disagree — the GUI previously hardcoded `true`.
pub fn os_is_64() -> bool {
    std::env::var("PROCESSOR_ARCHITECTURE").map(|a| a != "x86").unwrap_or(true)
        || std::env::var("PROCESSOR_ARCHITEW6432").is_ok()
}

/// Read a service's ImagePath from the registry.
///
/// Uses `collect::run_tool`, which already sets CREATE_NO_WINDOW — a bare
/// Command spawn from the GUI would stall the message pump.
/// Returns None for svchost-hosted services: the shared host's PE says
/// nothing about the bitness of the service DLL it loads.
#[cfg(windows)]
pub fn service_image_path(service: &str) -> Option<PathBuf> {
    let key = format!(r"{SERVICES_KEY}\{service}");
    let out = crate::collect::run_tool("reg.exe", &["query", &key, "/v", "ImagePath"]).ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);

    // reg.exe line: "    ImagePath    REG_EXPAND_SZ    C:\path\to.exe -args"
    // Locale caveat: matches the existing documented sc.exe assumption (en-US).
    let raw = text.lines().find_map(|l| {
        let l = l.trim();
        if !l.starts_with("ImagePath") {
            return None;
        }
        let after_name = l.strip_prefix("ImagePath")?.trim_start();
        let mut parts = after_name.splitn(2, char::is_whitespace);
        let _ty = parts.next()?; // REG_SZ / REG_EXPAND_SZ
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
    Some(path)
}

#[cfg(not(windows))]
pub fn service_image_path(_service: &str) -> Option<PathBuf> { None }

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
fn running_process_path(_name: &str) -> Option<PathBuf> { None }

/// Ordered bitness resolution. The returned &str names the source, for logs
/// and the GUI label.
///
///   1. PE header from the resolved path — correct even if the target has
///      never run, which is what makes `-w` work.
///   2. Runtime detection — covers a running process we could not path-resolve
///      (e.g. a svchost-hosted service).
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
```

Add `use crate::config::{Config, TargetType};` to the file's imports.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test bitness:: -- --nocapture
```

Expected: PASS. If `resolve_target_path` fails to compile on `cfg.target_path`, do Task 4 Step 3 first, then re-run.

- [ ] **Step 5: Commit**

```bash
git add rust/src/bitness.rs
git commit -m "feat(bitness): ordered target resolution (PE -> runtime -> unknown)

Service paths come from the registry ImagePath via collect::run_tool,
which already sets CREATE_NO_WINDOW. svchost-hosted services return None
so resolution falls through to runtime detection instead of reading the
shared host's PE. os_is_64() is extracted so the GUI and monitor cannot
disagree."
```

---

### Task 4: Persist the target's image path

**Files:**
- Modify: `rust/src/config.rs:46-101` (struct), `rust/src/config.rs:103+` (Default impl)
- Modify: `rust/src/gui/page_monitor.rs` (`on_target_picked`, `write_fields`)
- Test: `rust/src/config.rs`

**Interfaces:**
- Consumes: `bitness::resolve_target_path` (Task 3)
- Produces: `Config.target_path: String` (`TargetPath` on the wire)

The struct already carries `#[serde(default, rename_all = "PascalCase")]`, so an added field defaults automatically and old `config.json` files keep loading.

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `rust/src/config.rs`:

```rust
#[test]
fn target_path_defaults_empty_and_round_trips() {
    let mut c = Config::default();
    assert_eq!(c.target_path, "");
    c.target_path = r"C:\Program Files\SWH\CrossFire.Server.exe".into();
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains(r#""TargetPath""#), "must serialize PascalCase");
    let back: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(back.target_path, c.target_path);
}

#[test]
fn config_without_target_path_still_loads() {
    // An existing config.json predating this field must not fail to parse.
    let json = r#"{"ConfigVersion":1,"TargetName":"CrossFire","TargetType":"Service"}"#;
    let c: Config = serde_json::from_str(json).unwrap();
    assert_eq!(c.target_path, "");
    assert_eq!(c.target_name, "CrossFire");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test config::tests::target_path config::tests::config_without -- --nocapture
```

Expected: FAIL to compile — `no field 'target_path' on type 'Config'`.

- [ ] **Step 3: Implement**

In `rust/src/config.rs`, add to the struct immediately after `target_type` (line 49):

```rust
    /// Full image path of the target, captured when picked from the dropdown
    /// or resolved from a service's registry ImagePath. Lets bitness be read
    /// from the PE on disk when the target is not running (the `-w` case).
    pub target_path: String,
```

And in `impl Default for Config`, after `target_type: TargetType::Process,`:

```rust
            target_path: String::new(),
```

In `rust/src/gui/page_monitor.rs`, inside `write_fields` where `target_name`/`target_type` are written, add the path capture. Locate the existing assignment of the effective target and follow it with:

```rust
        // Capture the image path so bitness survives the target not running.
        // Only overwrite when we can actually resolve one — a failed lookup
        // must not erase a previously good path.
        if let Some(p) = crate::bitness::resolve_target_path(cfg) {
            cfg.target_path = p.to_string_lossy().to_string();
        }
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test -- --nocapture
```

Expected: PASS — the whole suite, since `Config` is widely constructed.

- [ ] **Step 5: Commit**

```bash
git add rust/src/config.rs rust/src/gui/page_monitor.rs
git commit -m "feat(config): persist TargetPath so bitness survives a stopped target

Struct-level serde(default) means configs predating this field still
load. A failed resolve does not erase a previously good path."
```

---

### Task 5: Re-resolve in the monitor loop, cached

**Files:**
- Modify: `rust/src/monitor.rs:37-48` and the `while` loop at `rust/src/monitor.rs:60`

**Interfaces:**
- Consumes: `bitness::{resolve, select_binary, os_is_64}` (Tasks 1–3)
- Produces: nothing consumed by later tasks

Today `detect()` runs **once** before the loop, so a target that starts later never corrects the choice. Re-resolving unconditionally would spawn `reg.exe` every cycle, and the cycle delay can be short — so cache, and only retry while the answer is still `Unknown`.

- [ ] **Step 1: Replace the one-shot selection**

Replace `rust/src/monitor.rs` lines 37-48 with:

```rust
    // Bitness-based binary switch (non-fatal on failure). Re-resolved while
    // Unknown so a target that starts later self-corrects; cached once known
    // so we do not spawn reg.exe every cycle.
    let pd_dir = Path::new(&cfg.proc_dump_path).parent()
        .map(|p| p.to_path_buf()).unwrap_or_else(paths::install_dir);
    let os64 = bitness::os_is_64();
    let mut resolved = bitness::Bitness::Unknown;
```

- [ ] **Step 2: Add the per-cycle re-resolution**

Immediately inside the `while !STOPPING...` loop, after `logger::log("Monitor", "-- Cycle start --");`:

```rust
        // Only re-resolve while still unknown — once known, it cannot change
        // without the config changing, and this can spawn reg.exe.
        if resolved == bitness::Bitness::Unknown {
            let (b, source) = bitness::resolve(&cfg);
            resolved = b;
            let choice = bitness::select_binary(b, &pd_dir, os64);
            if let Some(w) = &choice.warning {
                logger::log("Monitor", &format!("Bitness WARNING: {w}"));
            }
            // Compare the chosen BINARY PATH, not the source string: the
            // source can change while the selected binary does not.
            if choice.actual.exists() && choice.actual != Path::new(&cfg.proc_dump_path) {
                logger::log("Monitor",
                    &format!("Bitness: {} (via {source}) -> {}", choice.summary, choice.actual.display()));
                cfg.proc_dump_path = choice.actual.display().to_string();
                logger::log("Monitor", &format!("ProcDump args: {}", procdump::build_args(&cfg)));
            } else if b != bitness::Bitness::Unknown {
                logger::log("Monitor", &format!("Bitness: {} (via {source})", choice.summary));
            }
        }
```

- [ ] **Step 3: Build and verify it compiles**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" build 2>&1 | tail -20
```

Expected: compiles clean. Remove the now-unused `let choice = ...` / `logger::log("Monitor", &format!("Bitness: {}", choice.summary));` lines left over from the old block, and the now-redundant standalone `ProcDump args` log at old line 50 if it duplicates.

- [ ] **Step 4: Verify the whole suite still passes**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/monitor.rs
git commit -m "fix(monitor): re-resolve bitness until known instead of once at startup

detect() ran once before the loop, so with -w (default true) the target
was not running yet and every target fell back to procdump64.exe. Now
resolution retries each cycle while Unknown and caches once known, so a
target that starts later self-corrects without spawning reg.exe every
cycle. Logs compare the chosen binary path, not the source string."
```

---

### Task 6: Make the GUI label agree with the monitor

**Files:**
- Modify: `rust/src/gui/page_monitor.rs:480-491`

**Interfaces:**
- Consumes: `bitness::{resolve, select_binary, os_is_64}` (Tasks 1–3)
- Produces: nothing consumed by later tasks

`update_bitness` currently calls `detect(target)` and hardcodes `os_is_64: true`, so it can disagree with what the monitor will actually do — and on a 32-bit OS it lies.

- [ ] **Step 1: Rewrite `update_bitness`**

Replace lines 480-491 of `rust/src/gui/page_monitor.rs`:

```rust
    /// Shows the bitness the MONITOR will resolve, using the same code path,
    /// so the preview cannot disagree with runtime behaviour.
    fn update_bitness(&self, cfg: &Config, procdump_path: &str) {
        let pd_dir = std::path::Path::new(procdump_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(paths::install_dir);
        if cfg.target_name.trim().is_empty() {
            self.lbl_bitness.set_text("");
            return;
        }
        let (b, source) = bitness::resolve(cfg);
        let choice = bitness::select_binary(b, &pd_dir, bitness::os_is_64());
        let text = match (&choice.warning, b) {
            (Some(w), _) => format!("{} - {w}", choice.summary),
            (None, bitness::Bitness::Unknown) => format!(
                "{} - could not determine target bitness; verify manually.",
                choice.summary
            ),
            (None, _) => format!("{} (via {source})", choice.summary),
        };
        self.lbl_bitness.set_text(&text);
    }
```

- [ ] **Step 2: Update the two call sites**

`update_bitness` now takes `&Config` instead of `&str`. In `load()` (around line 583) replace:

```rust
        self.update_bitness(&cfg.target_name, &cfg.proc_dump_path);
```

with:

```rust
        self.update_bitness(cfg, &cfg.proc_dump_path);
```

In `on_target_picked` (around line 425) replace:

```rust
        self.update_bitness(&name, &self.txt_procdump_path.text());
```

with a control-pure clone — **use `write_fields`, never `save()`**, which would consume the typed webhook secret:

```rust
        let mut probe = state.cfg.borrow().clone();
        self.write_fields(&mut probe);
        self.update_bitness(&probe, &self.txt_procdump_path.text());
```

- [ ] **Step 3: Build and verify**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" build 2>&1 | tail -20
```

Expected: compiles clean. Fix any remaining `update_bitness` call sites the compiler reports.

- [ ] **Step 4: Run the suite**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" test 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/gui/page_monitor.rs
git commit -m "gui: bitness label uses the monitor's resolve() and real os_is_64

The label called detect() directly and hardcoded os_is_64: true, so it
could disagree with runtime behaviour and lied on a 32-bit OS. Probes via
a write_fields clone, never save(), which would consume the webhook."
```

---

### Task 7: Default the picker to the CrossFire server

**Files:**
- Modify: `rust/src/gui/page_monitor.rs:349-385` (`refresh_targets`), `:549+` (`load`)

**Interfaces:**
- Consumes: existing `TargetEntry { name: String, is_service: bool }`, `refresh_targets`, `load`
- Produces: `const PREFERRED_TARGETS: &[&str]`

Selection order: a saved config target wins (an explicit prior choice must never be overridden), then CrossFire if running, then the hint row. **Match exactly, not by substring** — four processes share the `SoftwareHouse.CrossFire.` prefix.

- [ ] **Step 1: Add the constant and the hint row**

Near the top of `rust/src/gui/page_monitor.rs`, by the other consts:

```rust
/// Shown at index 0 so a required field never reads as blank. CBS_DROPDOWNLIST
/// cannot render true placeholder text, so a real row is the Win32 answer.
const TARGET_HINT: &str = "- Select a process or service -";

/// Preferred default targets, exact (case-insensitive) exe-name match, highest
/// priority first. Used only when the config names no target.
///
/// EXACT match, not substring: SoftwareHouse.CrossFire.Server.exe,
/// .ServerComponentFramework.exe, .ImportWatcherService.exe and
/// .ReportServerService.exe all share the prefix, so `contains("crossfire")`
/// would pick whichever sorted first.
///
/// ponytail: one entry, no config-driven priority system. Add entries here if
/// other targets earn a default.
const PREFERRED_TARGETS: &[&str] = &["SoftwareHouse.CrossFire.Server.exe"];
```

- [ ] **Step 2: Insert the hint row in `refresh_targets`**

In `refresh_targets`, before the process loop populates `labels`/`entries`, push the hint as index 0 and keep `entries` index-aligned with `labels`:

```rust
        let mut entries: Vec<TargetEntry> = Vec::new();
        let mut labels: Vec<String> = Vec::new();

        // Index 0 is the hint row. It must occupy a slot in BOTH vectors so
        // combo indices stay aligned with `entries`; an empty name makes
        // effective_target() treat it as "nothing selected".
        labels.push(TARGET_HINT.to_string());
        entries.push(TargetEntry { name: String::new(), is_service: false });
```

Also treat the hint row as "no selection" when capturing the prior selection at the top of `refresh_targets`, so clicking Refresh picks up CrossFire once it starts rather than sticking on the hint. Replace:

```rust
        let selected = self.selected_entry().map(|e| e.name.clone());
```

with:

```rust
        // The hint row has an empty name; treat it as no selection so a
        // Refresh after CrossFire starts applies the default instead of
        // sticking on the hint.
        let selected = self
            .selected_entry()
            .map(|e| e.name.clone())
            .filter(|n| !n.is_empty());
```

Then, after `self.cmb_target.set_collection(labels);` and the existing "keep current selection" block, add the default:

```rust
        if let Some(sel) = selected {
            if let Some(i) = entries.iter().position(|e| e.name.eq_ignore_ascii_case(&sel)) {
                self.cmb_target.set_selection(Some(i));
            }
        } else {
            // No prior selection: prefer a known target if it is running,
            // else leave the hint row showing.
            let want = PREFERRED_TARGETS.iter().find_map(|p| {
                entries
                    .iter()
                    .position(|e| !e.is_service && e.name.eq_ignore_ascii_case(p))
            });
            self.cmb_target.set_selection(Some(want.unwrap_or(0)));
        }
        *self.entries.borrow_mut() = entries;
```

- [ ] **Step 3: Make `load` respect the hint row**

In `load()`, the saved-target lookup already skips non-matching entries. The hint row has an empty `name`, so `eq_ignore_ascii_case` against a non-empty saved name cannot match it. Add the fallback so an unmatched saved name still shows the hint rather than nothing:

```rust
        *self.manual_target.borrow_mut() =
            if found { String::new() } else { cfg.target_name.clone() };
        if !found && cfg.target_name.is_empty() {
            self.cmb_target.set_selection(Some(0)); // hint row
        }
```

- [ ] **Step 4: Build, then verify against the running app**

```bash
cd rust && PDM_TEST_MANIFEST=1 "$USERPROFILE/.cargo/bin/cargo.exe" build 2>&1 | tail -20
cd .. && powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/gui-e2e.ps1 \
  -Exe rust\target\debug\ProcDumpMonitor.exe -OutDir "$TMP/e2e-task7"
```

Expected: build clean; e2e exits 0. **The e2e asserts the first entry is `Proc:*`** (`gui-e2e.ps1:189`) — that assertion now needs to expect the hint row instead. Update it in the same commit:

```powershell
if ($first -ne '- Select a process or service -') { Fail "first target entry is not the hint row: '$first'" }
```

On a dev machine with no C·CURE installed, CrossFire will not be running, so the combo should show the hint row. That is the correct outcome here — do not "fix" it.

- [ ] **Step 5: Commit**

```bash
git add rust/src/gui/page_monitor.rs scripts/gui-e2e.ps1
git commit -m "gui: hint row at index 0 + default to CrossFire server when running

Picker was blank on load (CB_GETCURSEL -1) and CBS_DROPDOWNLIST cannot
render placeholder text, so index 0 is a real hint row. Default order:
saved config target, then SoftwareHouse.CrossFire.Server.exe if running,
then the hint. Exact match, not substring - four processes share the
SoftwareHouse.CrossFire. prefix."
```

---

### Task 8: Harden the e2e scroll probe and measure the size

**Files:**
- Modify: `scripts/gui-e2e.ps1:191-213`

**Interfaces:**
- Consumes: nothing
- Produces: nothing

The wheel probe has produced a false green (`CB_SETTOPINDEX`, shipped a bug) **and** a false red (foreground loss, 2026-07-26 — reported `0 -> 0` on a correct build, then `0 -> 45` on an identical re-run). `mouse_event` wheel goes to the FOREGROUND window, and `SetForegroundWindow` is refused when the caller does not own the foreground.

- [ ] **Step 1: Assert the preconditions before firing the wheel**

In `scripts/gui-e2e.ps1`, after the `SetCursorPos` call (line 204) and before reading `$topBefore`, add:

```powershell
# The wheel goes to the FOREGROUND window. If we could not foreground the
# app, a dead scroll is a HARNESS failure, not a product bug - say so.
$fg = [W.U32]::GetForegroundWindow()
if ($fg -ne $script:hwnd) { Fail "harness could not foreground the window (fg=$fg app=$($script:hwnd)) - scroll result is not trustworthy" }
$pt = New-Object W.PT; [W.U32]::GetCursorPos([ref]$pt) | Out-Null
$under = [W.U32]::WindowFromPoint($pt)
if ($under -ne $cbi.hwndList) { Fail "harness cursor is not over the dropdown list (under=$under list=$($cbi.hwndList)) - scroll result is not trustworthy" }
```

Add the P/Invoke declarations to the `W.U32` class if absent:

```csharp
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool GetCursorPos(ref PT p);
[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(PT p);
```

and the `PT` struct if absent:

```csharp
[StructLayout(LayoutKind.Sequential)] public struct PT { public int X,Y; }
```

- [ ] **Step 2: Run the e2e twice to confirm stability**

```bash
cd "C:/Users/mraburn/Documents/ProcDumpMonitor"
for i in 1 2; do
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/gui-e2e.ps1 \
    -Exe rust\target\debug\ProcDumpMonitor.exe -OutDir "$TMP/e2e-run$i" > "$TMP/e2e-run$i.log" 2>&1
  echo "run $i exit=$?"; tail -3 "$TMP/e2e-run$i.log"
done
```

Expected: both exit 0. **Capture the exit code directly — do NOT pipe to `tail`**, or you read `tail`'s status instead of PowerShell's.

- [ ] **Step 3: Release build and measure the size delta**

```bash
cd rust && "$USERPROFILE/.cargo/bin/cargo.exe" build --release 2>&1 | tail -5
ls -l target/release/ProcDumpMonitor.exe
```

Expected: builds (env `PDM_TEST_MANIFEST` must be UNSET or `build.rs` panics). Baseline is 2,063,872 bytes. **Report the delta against the ~2.0MB gate.** If it breaches, stop and report rather than silently trimming features.

- [ ] **Step 4: Confirm the release manifest**

```bash
cd rust && powershell.exe -NoProfile -Command "
  \$m = (Select-String -Path target/release/ProcDumpMonitor.exe -Pattern 'requireAdministrator' -Encoding Byte -AllMatches -ErrorAction SilentlyContinue)
  if (\$m) { 'requireAdministrator: PRESENT' } else { 'requireAdministrator: NOT FOUND - investigate' }"
```

Expected: PRESENT.

- [ ] **Step 5: Commit**

```bash
git add scripts/gui-e2e.ps1
git commit -m "verify: fail the scroll probe loudly when the harness loses foreground

The probe has thrown a false green (CB_SETTOPINDEX, shipped a bug) and a
false red (foreground loss reported 0 -> 0 on a correct build). Assert
GetForegroundWindow() == app and WindowFromPoint(cursor) == hwndList
before firing the wheel, so a harness failure never reads as a product
bug."
```

---

## Final Verification

- [ ] `PDM_TEST_MANIFEST=1 cargo test` — all pre-existing 63 tests plus the new PE, ImagePath, resolve, and config tests pass.
- [ ] `scripts/gui-e2e.ps1` exits 0 on two consecutive runs.
- [ ] Release exe size delta reported against the 2,063,872-byte baseline and the ~2.0MB gate.
- [ ] `requireAdministrator` confirmed in the release binary.
- [ ] **Field check the plan cannot perform here:** on a machine with C·CURE installed, confirm a `Svc:` target resolves to a real bitness (label reads `... (via PE header)`, not `could not determine target bitness`), and that `SoftwareHouse.NextGen.Client.MonitoringStation.exe` resolves to **X86 -> procdump.exe**. This dev machine has no C·CURE, so this is the one claim that cannot be closed here — report it as open.

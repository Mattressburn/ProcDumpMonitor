# ProcDumpMonitor Rust Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the .NET 8 WinForms app with a single ~3–6 MB native Rust exe (GUI wizard + headless CLI) that configures ProcDump as a SYSTEM scheduled task, runs the monitor loop, and sends email/webhook notifications.

**Architecture:** One binary, two faces: no args → nwg (Win32) 6-page wizard; verb args → headless CLI. Core modules are platform-gated so pure logic unit-tests run on Linux (`cd rust && cargo test`) while Windows-only code (DPAPI, schtasks, GUI, bitness) builds and tests on the win11-lab VM. Task registration uses `schtasks.exe /Create /XML` (no COM) with an XML format **proven by live spike** on the VM 2026-07-21.

**Tech Stack:** Rust 2021 (MSVC target), native-windows-gui 1.0.13, serde/serde_json, lettre (rustls), ureq, chrono, base64, windows crate (feature-gated), winresource (build).

**Spec:** `docs/superpowers/specs/2026-07-21-rust-rewrite-design.md` — read it first.

## Global Constraints

- Crate lives in `rust/` at repo root. The C# app stays untouched until the rewrite ships.
- Output exe name: `ProcDumpMonitor.exe` (`[[bin]] name = "ProcDumpMonitor"`).
- Windows floor: Windows 10 / Server 2016. Consequences: `IsWow64Process2` MUST be resolved dynamically via `GetProcAddress` (static import → exe fails to load on Server 2016); no WebView2 assumptions.
- Task Scheduler: via `schtasks.exe` only, never COM. Task XML principal is `<UserId>S-1-5-18</UserId>` + `<RunLevel>HighestAvailable</RunLevel>` with **no `<LogonType>` element** (proven: `ServiceAccount` fails validation). XML file written as **UTF-16LE with BOM**.
- DPAPI scope: LocalMachine (`CRYPTPROTECT_LOCAL_MACHINE`) — must decrypt under SYSTEM. Entropy strings verbatim: `ProcDumpMonitor-SMTP-v1`, `ProcDumpMonitor-Webhook-v1`.
- config.json / health.json field names must match the C# app exactly (PascalCase incl. `MemoryCommitMB`, `MinFreeDiskMB`, `MaxLogSizeMB`, `DumpRetentionMaxGB`, `FreeDiskMB`). Fresh schema, NO migration from deployed configs.
- No async runtime. No clap. Threads + std only for concurrency.
- Dependencies allowed: exactly those in Task 1's Cargo.toml. Adding any other crate requires user sign-off.
- Cut features (do NOT implement): --oneshot, --selftest, support-diagnostics ZIP, config export/import/migration, themes.
- All exe paths on the VM: source synced to `C:\pdm`, built artifact `C:\pdm\target\release\ProcDumpMonitor.exe`.
- VM access from LRPC: `ssh dev@192.168.69.110` (key auth, default shell PowerShell). Run PowerShell via `powershell -NoProfile -EncodedCommand <b64>` for anything with quoting (see `scripts/vm.sh` from Task 1).
- Commit after every green test cycle. Commit with `git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" commit ...` (repo has no git identity configured).

## File Structure

```
rust/
├── Cargo.toml
├── build.rs                  # winresource: icon + manifest; BUILD_DATE env
├── app.manifest              # requireAdministrator + common-controls v6
├── assets/jci_globe.ico      # copied from Assets/
└── src/
    ├── main.rs               # subsystem=windows, AttachConsole, GUI/CLI dispatch
    ├── paths.rs              # exe-adjacent config.json/health.json/Logs\
    ├── config.rs             # serde Config model, load/save, defaults
    ├── procdump.rs           # build_args + scenario presets
    ├── task.rs               # task XML gen (pure) + schtasks wrappers (windows)
    ├── logger.rs             # rotating file logger
    ├── health.rs             # HealthStatus + atomic write/load
    ├── retention.rs          # age + size dump retention
    ├── stability.rs          # dump file stability polling
    ├── diskguard.rs          # free-space check (windows) 
    ├── notify.rs             # email (lettre) + webhook (ureq) + queue thread
    ├── secrets.rs            # DPAPI protect/unprotect (windows)
    ├── bitness.rs            # select_binary (pure) + IsWow64Process2 detect (windows)
    ├── services.rs           # sc query exec (windows) + parser (pure)
    ├── cli.rs                # verb parsing + dispatch
    ├── monitor.rs            # the monitor loop (windows)
    └── gui/
        ├── mod.rs            # wizard window, step nav, page switching
        ├── page_target.rs    # Step 1
        ├── page_procdump.rs  # Step 2
        ├── page_task.rs      # Step 3
        ├── page_notify.rs    # Step 4
        ├── page_review.rs    # Step 5
        └── page_about.rs     # Step 6
scripts/
├── vm.sh                     # helper: run PowerShell on VM via EncodedCommand
└── vm-build.sh               # sync rust/ to VM, cargo build/test there, fetch exe
```

Module visibility: everything `pub(crate)`. Pure functions take plain params (no hidden globals) so they unit-test on Linux; Windows-only items are `#[cfg(windows)]`.

---

### Task 1: VM toolchain, crate scaffold, build pipeline, nwg+manifest spike

This task proves the spec's one unproven assumption (nwg + requireAdministrator manifest builds and runs elevated on the VM). **If the spike fails, STOP and report — the fallback is egui per the spec, which changes Task 9–11.**

**Files:**
- Create: `rust/Cargo.toml`, `rust/build.rs`, `rust/app.manifest`, `rust/src/main.rs`, `rust/assets/jci_globe.ico` (copy), `scripts/vm.sh`, `scripts/vm-build.sh`

**Interfaces:**
- Produces: working `cargo test` on Linux, working `scripts/vm-build.sh` that returns a built `ProcDumpMonitor.exe`, proven elevated nwg window.

- [ ] **Step 1: Write `scripts/vm.sh`** (PowerShell-over-SSH helper used by every VM step in this plan)

```bash
#!/usr/bin/env bash
# Usage: scripts/vm.sh 'powershell script text'   (multi-line OK, no quoting hell)
set -euo pipefail
VM="${VM:-dev@192.168.69.110}"
B64=$(printf '%s' "$1" | iconv -t UTF-16LE | base64 -w0)
ssh -o BatchMode=yes -o ConnectTimeout=5 "$VM" "powershell -NoProfile -EncodedCommand $B64" \
  | grep -v -a -i "post-quantum\|store now\|upgraded\|openssh.com\|^\*\* \|CLIXML\|^<Objs"
```

Run: `chmod +x scripts/vm.sh && scripts/vm.sh '"hello from vm"; whoami'`
Expected: `hello from vm` and `testvm\dev`

- [ ] **Step 2: Install Rust toolchain + MSVC Build Tools on the VM** (idempotent — skips if present)

```bash
scripts/vm.sh '
if (Get-Command cargo -ErrorAction SilentlyContinue) { "cargo already installed"; exit 0 }
winget install --id Rustlang.Rustup -e --silent --accept-package-agreements --accept-source-agreements
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --silent --accept-package-agreements --accept-source-agreements --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
"install kicked off"'
```

Then wait and verify (Build Tools takes 10–20 min; poll until green):

```bash
scripts/vm.sh '$env:Path = [Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [Environment]::GetEnvironmentVariable("Path","User"); cargo --version; rustup default stable-x86_64-pc-windows-msvc 2>&1; rustc --version'
```

Expected: `cargo 1.x` and `rustc 1.x` version lines. If `link.exe` errors appear later, re-check the VCTools workload finished (`Get-Process setup -ErrorAction SilentlyContinue` empty).

- [ ] **Step 3: Create the crate scaffold**

`rust/Cargo.toml`:

```toml
[package]
name = "procdumpmonitor"
version = "1.0.0"
edition = "2021"

[[bin]]
name = "ProcDumpMonitor"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", default-features = false, features = ["clock"] }
base64 = "0.22"
lettre = { version = "0.11", default-features = false, features = ["smtp-transport", "builder", "hostname", "pool", "rustls-tls"] }
ureq = { version = "2", features = ["json"] }

[target.'cfg(windows)'.dependencies]
native-windows-gui = "1.0.13"
native-windows-derive = "1.0.5"
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Security_Cryptography",
  "Win32_System_Threading",
  "Win32_System_Diagnostics_ToolHelp",
  "Win32_System_LibraryLoader",
  "Win32_Storage_FileSystem",
  "Win32_System_Console",
] }

[build-dependencies]
chrono = { version = "0.4", default-features = false, features = ["clock"] }

[target.'cfg(windows)'.build-dependencies]
winresource = "0.1"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

Note: `winresource` under `[target.'cfg(windows)'.build-dependencies]` gates on the HOST at build-script-compile time; since Windows builds happen natively on the VM (host = target = windows) and Linux only runs tests, this is correct.

`rust/build.rs`:

```rust
fn main() {
    println!(
        "cargo:rustc-env=BUILD_DATE={}",
        chrono::Local::now().format("%m.%d.%y")
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        #[cfg(windows)]
        {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/jci_globe.ico");
            res.set_manifest_file("app.manifest");
            res.compile().expect("resource compile failed");
        }
    }
}
```

`rust/app.manifest`:

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0" processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
    </windowsSettings>
  </application>
</assembly>
```

`rust/src/main.rs` (spike version — replaced in Task 8):

```rust
#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    native_windows_gui::init().expect("nwg init failed");
    let mut window = Default::default();
    let mut label = Default::default();
    native_windows_gui::Window::builder()
        .size((360, 120))
        .title("PDM Spike")
        .build(&mut window)
        .unwrap();
    native_windows_gui::Label::builder()
        .text("nwg + requireAdministrator OK")
        .size((320, 40))
        .position((20, 30))
        .parent(&window)
        .build(&mut label)
        .unwrap();
    let handler = native_windows_gui::full_bind_event_handler(
        &window.handle,
        move |evt, _data, _handle| {
            if evt == native_windows_gui::Event::OnWindowClose {
                native_windows_gui::stop_thread_dispatch();
            }
        },
    );
    native_windows_gui::dispatch_thread_events();
    native_windows_gui::unbind_event_handler(&handler);
}

#[cfg(not(windows))]
fn main() {
    eprintln!("ProcDumpMonitor targets Windows; Linux builds are for `cargo test` only.");
}
```

Copy the icon: `cp Assets/jci_globe.ico rust/assets/jci_globe.ico`

- [ ] **Step 4: Verify Linux side compiles**

Run: `cd rust && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors (nwg/windows crates skipped on Linux).

- [ ] **Step 5: Write `scripts/vm-build.sh`**

```bash
#!/usr/bin/env bash
# Sync rust/ to VM C:\pdm, run a cargo command there, fetch release exe if built.
# Usage: scripts/vm-build.sh [build|test|check]   (default: build --release)
set -euo pipefail
VM="${VM:-dev@192.168.69.110}"
CMD="${1:-build}"
cd "$(dirname "$0")/.."
tar czf /tmp/pdm-src.tgz --exclude=target -C rust .
scp -q -o BatchMode=yes /tmp/pdm-src.tgz "$VM:C:/Users/dev/pdm-src.tgz"
CARGO_ARGS="build --release"
[ "$CMD" = "test" ] && CARGO_ARGS="test"
[ "$CMD" = "check" ] && CARGO_ARGS="check"
scripts/vm.sh "
\$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
if (!(Test-Path C:\\pdm)) { mkdir C:\\pdm | Out-Null }
tar xzf C:\\Users\\dev\\pdm-src.tgz -C C:\\pdm
cd C:\\pdm
cargo $CARGO_ARGS 2>&1
\"CARGO_EXIT=\$LASTEXITCODE\""
if [ "$CMD" = "build" ]; then
  mkdir -p dist
  scp -q -o BatchMode=yes "$VM:C:/pdm/target/release/ProcDumpMonitor.exe" dist/ && \
    ls -la dist/ProcDumpMonitor.exe
fi
```

Run: `chmod +x scripts/vm-build.sh && scripts/vm-build.sh build`
Expected: cargo output ending `CARGO_EXIT=0`, then `dist/ProcDumpMonitor.exe` listed. First build compiles nwg — allow a few minutes.

- [ ] **Step 6: THE SPIKE — run the exe elevated on the VM and verify the window + elevation**

The SSH session cannot show a UAC prompt; `dev` is admin, and over SSH processes run with a full (elevated) token, so launching directly verifies the manifest doesn't crash the loader and the window opens:

```bash
scripts/vm.sh '
$p = Start-Process C:\pdm\target\release\ProcDumpMonitor.exe -PassThru
Start-Sleep -Seconds 3
$alive = !$p.HasExited
"ALIVE=$alive"
if ($alive) { Stop-Process -Id $p.Id -Force; "window stayed open - spike PASS" } else { "exited code $($p.ExitCode) - spike FAIL" }'
```

Expected: `ALIVE=True` and `spike PASS`.
Then confirm the manifest is actually embedded (requireAdministrator present):

```bash
scripts/vm.sh '
$sig = Select-String -Path C:\pdm\target\release\ProcDumpMonitor.exe -Pattern "requireAdministrator" -SimpleMatch -Quiet
"MANIFEST_EMBEDDED=$sig"'
```

Expected: `MANIFEST_EMBEDDED=True`. **If either check fails: STOP, report to user, propose egui fallback per spec.**

- [ ] **Step 7: Commit**

```bash
git add rust/ scripts/vm.sh scripts/vm-build.sh
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: scaffold, VM build pipeline, nwg+manifest spike passes"
```

---

### Task 2: paths + config module

**Files:**
- Create: `rust/src/paths.rs`, `rust/src/config.rs`
- Modify: `rust/src/main.rs` (add `mod` declarations)

**Interfaces:**
- Produces: `paths::install_dir() -> PathBuf`, `paths::config_path() -> PathBuf`, `paths::health_path() -> PathBuf`, `paths::log_path() -> PathBuf`, `paths::exe_path() -> PathBuf`;
  `config::Config` (all fields below), `config::TargetType { Process, Service }`, `Config::default()`, `Config::load(path: &Path) -> Config`, `Config::save(&mut self, path: &Path) -> std::io::Result<()>`.
- JSON field names are the C# names verbatim (see Global Constraints).

- [ ] **Step 1: Write failing tests** (bottom of `config.rs`)

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test config 2>&1 | tail -5`
Expected: compile error — `Config` not defined.

- [ ] **Step 3: Implement**

`rust/src/paths.rs`:

```rust
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
```

`rust/src/config.rs`:

```rust
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
```

In `main.rs` add above `fn main`:

```rust
mod config;
mod paths;
```

- [ ] **Step 4: Run tests**

Run: `cd rust && cargo test config 2>&1 | tail -5`
Expected: `test result: ok. 3 passed`

- [ ] **Step 5: Commit**

```bash
git add rust/src/config.rs rust/src/paths.rs rust/src/main.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: config model + paths (C#-compatible JSON schema)"
```

---

### Task 3: procdump args + scenario presets

**Files:**
- Create: `rust/src/procdump.rs`
- Modify: `rust/src/main.rs` (`mod procdump;`)

**Interfaces:**
- Consumes: `config::{Config, TargetType}`.
- Produces: `procdump::build_args(cfg: &Config) -> String`;
  `procdump::Preset { pub name: &'static str, pub description: &'static str, pub effective_flags: &'static str }` with `Preset::all() -> &'static [Preset]`, `Preset::find(name: &str) -> Option<&'static Preset>`, `Preset::apply(&self, cfg: &mut Config)`.
- Flag order MUST match the C# `BuildProcDumpArgs` exactly (tests below encode it).

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, TargetType};

    fn base() -> Config {
        let mut c = Config::default();
        c.target_name = "MyApp".into();
        c.dump_directory = r"C:\Dumps\MyApp".into();
        c
    }

    #[test]
    fn default_crash_args_match_csharp_order() {
        let c = base();
        // defaults: Full, -e, -t, -r, -n 1, -w, target gets .exe appended, quoted dir
        assert_eq!(
            build_args(&c),
            r#"-accepteula -ma -e -t -r -n 1 -w MyApp.exe "C:\Dumps\MyApp""#
        );
    }

    #[test]
    fn service_target_uses_service_flag_and_no_exe_suffix() {
        let mut c = base();
        c.target_type = TargetType::Service;
        assert!(build_args(&c).contains("-w -service MyApp \""));
    }

    #[test]
    fn exe_suffix_not_doubled() {
        let mut c = base();
        c.target_name = "MyApp.EXE".into();
        assert!(build_args(&c).contains("-w MyApp.EXE \""));
    }

    #[test]
    fn all_flags_render_in_order() {
        let mut c = base();
        c.dump_type = "MiniPlus".into();
        c.hang_window_seconds = 1;
        c.avoid_outage = true;
        c.overwrite_existing = true;
        c.cpu_threshold = 90;
        c.cpu_low_threshold = 5;
        c.cpu_duration_seconds = 10;
        c.cpu_per_unit = true;
        c.memory_commit_mb = 2048;
        c.performance_counter = r"\Processor(_Total)\% Processor Time".into();
        c.exception_filter_include = "OutOfMemory".into();
        c.wer_integration = true;
        c.avoid_terminate_timeout = 7;
        c.max_dumps = 3;
        let a = build_args(&c);
        assert_eq!(
            a,
            r#"-accepteula -mp -e -t -h -r -a -o -c 90 -cl 5 -s 10 -u -m 2048 -p "\Processor(_Total)\% Processor Time" -f "OutOfMemory" -wer -at 7 -n 3 -w MyApp.exe "C:\Dumps\MyApp""#
        );
    }

    #[test]
    fn presets_match_readme_flags() {
        let mut c = base();
        Preset::find("High CPU spike capture").unwrap().apply(&mut c);
        assert_eq!(c.cpu_threshold, 90);
        assert_eq!(c.cpu_duration_seconds, 10);
        assert_eq!(c.max_dumps, 3);
        assert!(!c.dump_on_exception, "preset reset must zero triggers");
        // reset preserves paths + wait_for_process
        assert_eq!(c.dump_directory, r"C:\Dumps\MyApp");
        assert!(c.wait_for_process);
        assert_eq!(Preset::all().len(), 5);
        assert_eq!(Preset::all()[0].name, "Crash capture");
        assert_eq!(Preset::find("Low impact full dump").unwrap().effective_flags, "-a -r -ma");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test procdump 2>&1 | tail -5`
Expected: compile error — `build_args` not defined.

- [ ] **Step 3: Implement**

```rust
use crate::config::{Config, TargetType};

/// Port of C# Config.BuildProcDumpArgs — flag order is contract, do not "tidy".
pub fn build_args(cfg: &Config) -> String {
    let mut args: Vec<String> = vec!["-accepteula".into()];

    match cfg.dump_type.as_str() {
        "Full" => args.push("-ma".into()),
        "MiniPlus" => args.push("-mp".into()),
        "Mini" => args.push("-mm".into()),
        "ThreadDump" => args.push("-mt".into()),
        _ => {}
    }

    if cfg.dump_on_exception { args.push("-e".into()); }
    if cfg.dump_on_terminate { args.push("-t".into()); }
    if cfg.hang_window_seconds > 0 { args.push("-h".into()); }

    if cfg.use_clone { args.push("-r".into()); }
    if cfg.avoid_outage { args.push("-a".into()); }
    if cfg.overwrite_existing { args.push("-o".into()); }

    if cfg.cpu_threshold > 0 { args.push(format!("-c {}", cfg.cpu_threshold)); }
    if cfg.cpu_low_threshold > 0 { args.push(format!("-cl {}", cfg.cpu_low_threshold)); }
    if cfg.cpu_duration_seconds > 0 { args.push(format!("-s {}", cfg.cpu_duration_seconds)); }
    if cfg.cpu_per_unit { args.push("-u".into()); }

    if cfg.memory_commit_mb > 0 { args.push(format!("-m {}", cfg.memory_commit_mb)); }

    if !cfg.performance_counter.trim().is_empty() {
        args.push(format!("-p \"{}\"", cfg.performance_counter));
    }
    if !cfg.perf_counter_threshold.trim().is_empty() {
        args.push(format!("-pl \"{}\"", cfg.perf_counter_threshold));
    }
    if !cfg.exception_filter_include.trim().is_empty() {
        args.push(format!("-f \"{}\"", cfg.exception_filter_include));
    }
    if !cfg.exception_filter_exclude.trim().is_empty() {
        args.push(format!("-fx \"{}\"", cfg.exception_filter_exclude));
    }
    if cfg.wer_integration { args.push("-wer".into()); }
    if cfg.avoid_terminate_timeout > 0 { args.push(format!("-at {}", cfg.avoid_terminate_timeout)); }

    args.push(format!("-n {}", cfg.max_dumps));
    if cfg.wait_for_process { args.push("-w".into()); }

    match cfg.target_type {
        TargetType::Service => args.push(format!("-service {}", cfg.target_name)),
        TargetType::Process => {
            let t = cfg.target_name.clone();
            if !t.trim().is_empty() && !t.to_ascii_lowercase().ends_with(".exe") {
                args.push(format!("{t}.exe"));
            } else {
                args.push(t);
            }
        }
    }

    args.push(format!("\"{}\"", cfg.dump_directory));
    args.join(" ")
}

pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub effective_flags: &'static str,
    apply_fn: fn(&mut Config),
}

impl Preset {
    pub fn all() -> &'static [Preset] { &PRESETS }

    pub fn find(name: &str) -> Option<&'static Preset> {
        PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Reset all trigger/operational fields to safe zeroes, then apply.
    /// Preserves: WaitForProcess, ProcDumpPath, DumpDirectory, TargetName, RestartDelay.
    pub fn apply(&self, cfg: &mut Config) {
        cfg.dump_type = "Full".into();
        cfg.dump_on_exception = false;
        cfg.dump_on_terminate = false;
        cfg.use_clone = false;
        cfg.avoid_outage = false;
        cfg.overwrite_existing = false;
        cfg.cpu_per_unit = false;
        cfg.cpu_threshold = 0;
        cfg.cpu_low_threshold = 0;
        cfg.cpu_duration_seconds = 0;
        cfg.memory_commit_mb = 0;
        cfg.hang_window_seconds = 0;
        cfg.max_dumps = 1;
        cfg.wer_integration = false;
        cfg.avoid_terminate_timeout = 0;
        cfg.performance_counter.clear();
        cfg.perf_counter_threshold.clear();
        cfg.exception_filter_include.clear();
        cfg.exception_filter_exclude.clear();
        (self.apply_fn)(cfg);
        cfg.scenario = self.name.into();
    }
}

static PRESETS: [Preset; 5] = [
    Preset {
        name: "Crash capture",
        description: "Captures a full dump when the process throws an unhandled exception or terminates unexpectedly. Uses safe defaults appropriate for production systems. Ideal for post-mortem crash investigation.",
        effective_flags: "-ma -e -t",
        apply_fn: |c| { c.dump_on_exception = true; c.dump_on_terminate = true; },
    },
    Preset {
        name: "Hang capture",
        description: "Captures a full dump when the process window stops responding (hung). Useful for diagnosing UI freezes and deadlocks.",
        effective_flags: "-ma -h",
        apply_fn: |c| { c.hang_window_seconds = 1; },
    },
    Preset {
        name: "High CPU spike capture",
        description: "Captures up to 3 full dumps when CPU usage exceeds 90 % for at least 10 consecutive seconds. Helps identify runaway threads or hot code paths.",
        effective_flags: "-ma -c 90 -s 10 -n 3",
        apply_fn: |c| { c.cpu_threshold = 90; c.cpu_duration_seconds = 10; c.max_dumps = 3; },
    },
    Preset {
        name: "Memory threshold capture",
        description: "Captures up to 3 full dumps when process memory commit exceeds 2048 MB. Useful for investigating memory leaks or unexpected memory growth.",
        effective_flags: "-ma -m 2048 -n 3",
        apply_fn: |c| { c.memory_commit_mb = 2048; c.max_dumps = 3; },
    },
    Preset {
        name: "Low impact full dump",
        description: "A full memory dump equivalent to Task Manager, captured via process cloning (-r) to minimize disruption. The -a flag prevents dump floods; the process is suspended for only milliseconds instead of the full dump duration.",
        effective_flags: "-a -r -ma",
        apply_fn: |c| { c.avoid_outage = true; c.use_clone = true; c.max_dumps = 1; },
    },
];
```

Note: unlike the C# original, `apply` also stamps `cfg.scenario` — the GUI relied on doing that separately; folding it in removes a footgun.

- [ ] **Step 4: Run tests**

Run: `cd rust && cargo test procdump 2>&1 | tail -5`
Expected: `test result: ok. 5 passed`

- [ ] **Step 5: Commit**

```bash
git add rust/src/procdump.rs rust/src/main.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: procdump arg builder + 5 scenario presets"
```

---

### Task 4: task module — XML generation (pure) + schtasks wrappers (windows)

**Files:**
- Create: `rust/src/task.rs`
- Modify: `rust/src/main.rs` (`mod task;`)

**Interfaces:**
- Consumes: `config::Config`, `paths::{exe_path, config_path, install_dir}`.
- Produces (pure, Linux-tested):
  `task::sanitize_task_name(&str) -> String` (strips `\ / : * ? " < > |`),
  `task::auto_task_name(target: &str) -> String` (`"ProcDump Monitor {target}"`),
  `task::task_xml(target_name: &str, exe: &str, config_path: &str, workdir: &str) -> String`,
  `task::to_utf16le_bom(&str) -> Vec<u8>`,
  `task::xml_escape(&str) -> String`.
- Produces (`#[cfg(windows)]`, VM-tested in Task 8):
  `task::install(cfg: &Config) -> Result<bool, String>` (Ok(true) = updated existing),
  `task::uninstall(task_name: &str) -> Result<(), String>`,
  `task::start(task_name: &str) -> Result<(), String>`,
  `task::stop(task_name: &str) -> Result<(), String>`,
  `task::exists(task_name: &str) -> bool`,
  `task::query_status(task_name: &str) -> TaskStatus`,
  `pub struct TaskStatus { pub exists: bool, pub state: String, pub last_run_time: String, pub last_run_result: String, pub next_run_time: String }`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_reserved_chars() {
        assert_eq!(sanitize_task_name(r#"a\b/c:d*e?f"g<h>i|j"#), "abcdefghij");
        assert_eq!(auto_task_name("MyApp"), "ProcDump Monitor MyApp");
    }

    #[test]
    fn xml_matches_proven_spike_structure() {
        let xml = task_xml("MyApp", r"C:\Tools\ProcDumpMonitor.exe",
                           r"C:\Tools\config.json", r"C:\Tools");
        // Landmines proven on the VM 2026-07-21:
        assert!(!xml.contains("<LogonType>"), "LogonType must be omitted for SYSTEM");
        assert!(xml.contains("<UserId>S-1-5-18</UserId>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<BootTrigger>"));
        assert!(xml.contains("<Interval>PT1M</Interval>"));
        assert!(xml.contains("<Count>999</Count>"));
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<StartWhenAvailable>true</StartWhenAvailable>"));
        assert!(xml.contains(r"<Command>C:\Tools\ProcDumpMonitor.exe</Command>"));
        assert!(xml.contains(r#"<Arguments>--monitor --config "C:\Tools\config.json"</Arguments>"#));
        assert!(xml.contains(r"<WorkingDirectory>C:\Tools</WorkingDirectory>"));
        assert!(xml.contains("watches for MyApp"));
    }

    #[test]
    fn xml_escapes_special_chars() {
        let xml = task_xml("A&B", r"C:\Tools & Co\p.exe", r"C:\x.json", r"C:\y");
        assert!(xml.contains("A&amp;B"));
        assert!(xml.contains(r"C:\Tools &amp; Co\p.exe"));
    }

    #[test]
    fn utf16le_bom_encoding() {
        let bytes = to_utf16le_bom("<a/>");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE], "BOM required");
        assert_eq!(bytes.len(), 2 + 2 * 4);
        assert_eq!(&bytes[2..4], &[b'<', 0x00]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test task 2>&1 | tail -5`
Expected: compile error.

- [ ] **Step 3: Implement the pure half**

```rust
use crate::config::Config;

pub fn sanitize_task_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect()
}

pub fn auto_task_name(target: &str) -> String {
    format!("ProcDump Monitor {target}")
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;").replace('\'', "&apos;")
}

pub fn to_utf16le_bom(s: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// Task Scheduler XML, format proven by live spike on win11-lab 2026-07-21.
/// SYSTEM principal: UserId only, NO LogonType (schtasks rejects ServiceAccount).
pub fn task_xml(target_name: &str, exe: &str, config_path: &str, workdir: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>ProcDump Monitor - watches for {target} and captures crash dumps.</Description>
  </RegistrationInfo>
  <Triggers>
    <BootTrigger>
      <Enabled>true</Enabled>
    </BootTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-18</UserId>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>true</Enabled>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>--monitor --config &quot;{config}&quot;</Arguments>
      <WorkingDirectory>{workdir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
        target = xml_escape(target_name),
        exe = xml_escape(exe),
        config = xml_escape(config_path),
        workdir = xml_escape(workdir),
    )
}
```

Note the `&quot;` around the config path — after XML unescaping the task action carries real quotes: `--monitor --config "C:\...\config.json"`. The test asserts on the *escaped* form via `xml_escape` output; write the assertion against what `task_xml` actually emits (`&quot;` form) — adjust the test's `Arguments` assertion to:
`assert!(xml.contains(r#"<Arguments>--monitor --config &quot;C:\Tools\config.json&quot;</Arguments>"#));`

- [ ] **Step 4: Run tests**

Run: `cd rust && cargo test task 2>&1 | tail -5`
Expected: `test result: ok. 4 passed`

- [ ] **Step 5: Implement the Windows half** (compiles now, exercised on VM in Task 8)

Append to `task.rs`:

```rust
#[derive(Debug, Default, serde::Serialize)]
pub struct TaskStatus {
    #[serde(rename = "TaskName")] pub task_name: String,
    #[serde(rename = "MachineName")] pub machine_name: String,
    #[serde(rename = "Exists")] pub exists: bool,
    #[serde(rename = "State")] pub state: String,
    #[serde(rename = "LastRunTime")] pub last_run_time: String,
    #[serde(rename = "LastRunResult")] pub last_run_result: String,
    #[serde(rename = "NextRunTime")] pub next_run_time: String,
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::{logger, paths};
    use std::process::Command;

    fn schtasks(args: &[&str]) -> Result<String, String> {
        let out = Command::new("schtasks.exe")
            .args(args)
            .output()
            .map_err(|e| format!("cannot run schtasks: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if out.status.success() {
            Ok(stdout)
        } else {
            Err(if stderr.trim().is_empty() { stdout } else { stderr })
        }
    }

    /// Create-or-update. Returns Ok(true) if a task with this name already existed.
    pub fn install(cfg: &Config) -> Result<bool, String> {
        let name = sanitize_task_name(&cfg.task_name);
        if name.trim().is_empty() {
            return Err("Task name is empty after sanitisation.".into());
        }
        let exe = paths::exe_path().display().to_string();
        let config_path = paths::config_path().display().to_string();
        let workdir = paths::install_dir().display().to_string();
        let existed = exists(&name);

        let xml = task_xml(&cfg.target_name, &exe, &config_path, &workdir);
        let xml_file = std::env::temp_dir().join("pdm_task.xml");
        std::fs::write(&xml_file, to_utf16le_bom(&xml))
            .map_err(|e| format!("cannot write task xml: {e}"))?;

        let res = schtasks(&["/Create", "/TN", &name, "/XML",
                             &xml_file.display().to_string(), "/F"]);
        let _ = std::fs::remove_file(&xml_file);
        res?;
        logger::log("TaskSvc", &format!("Task '{name}' registered (existed={existed})."));
        Ok(existed)
    }

    pub fn uninstall(task_name: &str) -> Result<(), String> {
        schtasks(&["/Delete", "/TN", task_name, "/F"]).map(|_| ())
    }

    pub fn start(task_name: &str) -> Result<(), String> {
        schtasks(&["/Run", "/TN", task_name]).map(|_| ())
    }

    pub fn stop(task_name: &str) -> Result<(), String> {
        schtasks(&["/End", "/TN", task_name]).map(|_| ())
    }

    pub fn exists(task_name: &str) -> bool {
        schtasks(&["/Query", "/TN", task_name]).is_ok()
    }

    /// Parses `/Query /V /FO CSV` positionally (headers are localized; the
    /// column ORDER is stable): 0=HostName 1=TaskName 2=NextRunTime 3=Status
    /// 5=LastRunTime 6=LastResult.
    pub fn query_status(task_name: &str) -> TaskStatus {
        let mut st = TaskStatus {
            task_name: task_name.into(),
            machine_name: std::env::var("COMPUTERNAME").unwrap_or_default(),
            state: "Not installed".into(),
            ..Default::default()
        };
        let Ok(csv) = schtasks(&["/Query", "/TN", task_name, "/V", "/FO", "CSV"]) else {
            return st;
        };
        let Some(data_line) = csv.lines().nth(1) else { return st };
        let cols = parse_csv_line(data_line);
        if cols.len() > 6 {
            st.exists = true;
            st.next_run_time = cols[2].clone();
            st.state = cols[3].clone();
            st.last_run_time = cols[5].clone();
            st.last_run_result = cols[6].clone();
        }
        st
    }
}

#[cfg(windows)]
pub use win::*;

/// Minimal CSV field splitter for schtasks output (quoted fields, comma sep).
pub fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => { cur.push('"'); chars.next(); }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => { fields.push(std::mem::take(&mut cur)); }
            _ => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}
```

Add a Linux-runnable test for the CSV splitter to the test module:

```rust
    #[test]
    fn csv_line_parsing_handles_quotes() {
        let cols = parse_csv_line(r#""HOST","\PDM Task","N/A","Ready","x","07/21/2026 4:00:00 PM","0""#);
        assert_eq!(cols[1], r"\PDM Task");
        assert_eq!(cols[3], "Ready");
        assert_eq!(cols[6], "0");
    }
```

- [ ] **Step 6: Run tests + Linux check, commit**

Run: `cd rust && cargo test task 2>&1 | tail -5 && cargo check 2>&1 | tail -3`
Expected: `5 passed`, check clean (win module is cfg'd out on Linux; `logger` doesn't exist yet — if that breaks check, defer the `logger::log` line to Task 5 and use a `// TODO(task5)`-free plain comment; simplest is to add `pub mod logger` stub in Task 5's commit order — instead: implement Task 5 FIRST if compile fails. To keep this task self-contained, replace the `logger::log(...)` call with nothing for now; Task 8 wires logging).

```bash
git add rust/src/task.rs rust/src/main.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: task XML (spike-proven format) + schtasks wrappers"
```

---

### Task 5: monitor support — logger, health, retention, stability, diskguard

**Files:**
- Create: `rust/src/logger.rs`, `rust/src/health.rs`, `rust/src/retention.rs`, `rust/src/stability.rs`, `rust/src/diskguard.rs`
- Modify: `rust/src/main.rs` (mod declarations), `rust/src/task.rs` (restore `logger::log` call if deferred in Task 4)

**Interfaces:**
- Produces:
  `logger::init(path: PathBuf, max_size_mb: i32, max_files: i32)` and `logger::log(category: &str, msg: &str)` (no-op safe if uninitialized);
  `health::HealthStatus` (C# field names: MonitorPid, ProcDumpPid, LastCycleUtc, LastProcDumpExitCode, LastDumpFileName, TotalDumpCount, LastError, NextRetryUtc, LastNotifiedDumpFile, LastNotifiedUtc, DiskSpaceLow, FreeDiskMB, Version), `health::write(path, &HealthStatus)` (atomic: tmp + rename), `health::load(path) -> HealthStatus`;
  `retention::apply(dir: &Path, retention_days: i32, max_gb: f64) -> usize`;
  `stability::wait_for_stable_file(path: &Path, timeout_s: i32, poll_s: i32) -> bool`;
  `diskguard::check_free_space(path: &Path, min_free_mb: i64) -> (bool, i64)`.

- [ ] **Step 1: Write failing tests**

`retention.rs` tests (std `File::set_modified`, Rust ≥1.75):

```rust
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
```

`health.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_csharp_names() {
        let dir = std::env::temp_dir().join("pdm_health");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("health.json");
        let mut h = HealthStatus::default();
        h.total_dump_count = 7;
        h.last_notified_dump_file = "x.dmp".into();
        write(&p, &h);
        let json = std::fs::read_to_string(&p).unwrap();
        for k in ["\"MonitorPid\"", "\"ProcDumpPid\"", "\"TotalDumpCount\"",
                  "\"LastNotifiedDumpFile\"", "\"FreeDiskMB\"", "\"DiskSpaceLow\""] {
            assert!(json.contains(k), "missing {k}");
        }
        let loaded = load(&p);
        assert_eq!(loaded.total_dump_count, 7);
        // missing/corrupt -> default
        assert_eq!(load(&dir.join("nope.json")).total_dump_count, 0);
    }
}
```

`stability.rs` test (cross-platform size-stability logic; exclusive-lock check is windows-only):

```rust
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
```

- [ ] **Step 2: Run to verify failures**

Run: `cd rust && cargo test 'retention|health|stability' 2>&1 | tail -5` — expected: compile errors.

- [ ] **Step 3: Implement**

`rust/src/logger.rs`:

```rust
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
```

`rust/src/health.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct HealthStatus {
    pub monitor_pid: u32,
    pub proc_dump_pid: u32,
    pub last_cycle_utc: String,
    pub last_proc_dump_exit_code: i32,
    pub last_dump_file_name: String,
    pub total_dump_count: i32,
    pub last_error: String,
    pub next_retry_utc: String,
    pub last_notified_dump_file: String,
    pub last_notified_utc: String,
    pub disk_space_low: bool,
    #[serde(rename = "FreeDiskMB")]
    pub free_disk_mb: i64,
    pub version: String,
}

/// Atomic write (tmp + rename) so monitors never read a torn file. Never panics.
pub fn write(path: &Path, status: &HealthStatus) {
    let Ok(json) = serde_json::to_string_pretty(status) else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

pub fn load(path: &Path) -> HealthStatus {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}
```

`rust/src/retention.rs`:

```rust
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
```

`rust/src/stability.rs`:

```rust
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
```

`rust/src/diskguard.rs`:

```rust
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
```

Add to `main.rs`: `mod logger; mod health; mod retention; mod stability; mod diskguard;`
Restore the `logger::log` call in `task.rs::install` if it was deferred in Task 4.

- [ ] **Step 4: Run all tests**

Run: `cd rust && cargo test 2>&1 | tail -5`
Expected: all green (config 3, procdump 5, task 5, retention 3, health 1, stability 3). The growing-file test takes ~5 s.

- [ ] **Step 5: Commit**

```bash
git add rust/src/{logger,health,retention,stability,diskguard}.rs rust/src/main.rs rust/src/task.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: logger, health, retention, stability, diskguard"
```

---

### Task 6: notify — email (lettre) + webhook (ureq) + background queue

**Files:**
- Create: `rust/src/notify.rs`
- Modify: `rust/src/main.rs` (`mod notify;`)

**Interfaces:**
- Consumes: `config::Config`, `secrets` (Task 7 — until then use `cfg.encrypted_password_blob` via a `decrypt_password` seam, see Step 3), `logger`.
- Produces:
  `notify::split_addresses(&str) -> Vec<String>`,
  `notify::TlsMode { Wrapper, Required, Opportunistic }` + `notify::tls_mode(use_ssl: bool, port: u16) -> TlsMode`,
  `notify::dump_email(target: &str, machine: &str, dump_path: &str) -> (String, String)` (subject, body),
  `notify::webhook_payload_dump(target: &str, machine: &str, dump_path: &str) -> WebhookPayload` and `notify::webhook_payload_warning(subject: &str, message: &str) -> WebhookPayload`,
  `notify::send_email(cfg: &Config, subject: &str, body: &str) -> Result<(), String>`,
  `notify::send_test_email(cfg: &Config) -> Result<(), String>`,
  `notify::post_webhook(url: &str, payload: &WebhookPayload)` (logs, never fails outward),
  `notify::validate_smtp_connectivity(server: &str, port: u16, timeout_ms: u64) -> (bool, String)` (raw TCP + banner),
  `notify::NotifyQueue` with `NotifyQueue::new() -> Self`, `enqueue_dump(&self, cfg: Config, dump_path: String)`, `enqueue_warning(&self, cfg: Config, subject: String, message: String)` (bounded 64, drops + logs when full).

- [ ] **Step 1: Write failing tests** (all pure — run on Linux)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_splitting() {
        assert_eq!(split_addresses(" a@x.com ; ;b@y.com;"), vec!["a@x.com", "b@y.com"]);
        assert!(split_addresses("").is_empty());
    }

    #[test]
    fn tls_mode_selection_matches_csharp() {
        assert_eq!(tls_mode(true, 465), TlsMode::Wrapper);
        assert_eq!(tls_mode(true, 587), TlsMode::Required);
        assert_eq!(tls_mode(false, 25), TlsMode::Opportunistic);
    }

    #[test]
    fn dump_email_format() {
        let (subject, body) = dump_email("MyApp", "SERVER01", r"C:\Dumps\MyApp_1.dmp");
        assert_eq!(subject, "[ProcDump] Dump created for MyApp on SERVER01");
        assert!(body.contains("Target:     MyApp"));
        assert!(body.contains("Computer:   SERVER01"));
        assert!(body.contains(r"Dump File:  C:\Dumps\MyApp_1.dmp"));
        assert!(body.contains("Timestamp:  "));
    }

    #[test]
    fn webhook_payload_is_messagecard() {
        let p = webhook_payload_dump("MyApp", "SERVER01", r"C:\d\x.dmp");
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""@type":"MessageCard""#));
        assert!(json.contains(r#""themeColor":"FF0000""#));
        assert!(json.contains("Dump created for MyApp"));
        let w = webhook_payload_warning("subj", "msg");
        assert_eq!(w.theme_color, "FFAA00");
    }

    #[test]
    fn queue_executes_work_and_survives_panic() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let q = NotifyQueue::new();
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = n.clone();
        q.enqueue(Box::new(move || { n2.fetch_add(1, Ordering::SeqCst); }));
        q.enqueue(Box::new(|| panic!("notifier blew up")));
        let n3 = n.clone();
        q.enqueue(Box::new(move || { n3.fetch_add(1, Ordering::SeqCst); }));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(n.load(Ordering::SeqCst), 2, "work after a panicking job must still run");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test notify 2>&1 | tail -5` — expected: compile errors.

- [ ] **Step 3: Implement**

```rust
use crate::config::Config;
use crate::logger;
use serde::Serialize;

pub fn split_addresses(s: &str) -> Vec<String> {
    s.split(';').map(str::trim).filter(|a| !a.is_empty()).map(String::from).collect()
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TlsMode { Wrapper, Required, Opportunistic }

/// C# rule: UseSsl+465 = implicit SSL; UseSsl+other = STARTTLS; else opportunistic.
pub fn tls_mode(use_ssl: bool, port: u16) -> TlsMode {
    if use_ssl {
        if port == 465 { TlsMode::Wrapper } else { TlsMode::Required }
    } else {
        TlsMode::Opportunistic
    }
}

pub fn machine_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into())
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn dump_email(target: &str, machine: &str, dump_path: &str) -> (String, String) {
    let subject = format!("[ProcDump] Dump created for {target} on {machine}");
    let body = format!(
        "A process dump was captured.\r\n\r\n\
         Target:     {target}\r\n\
         Computer:   {machine}\r\n\
         Dump File:  {dump_path}\r\n\
         Timestamp:  {}\r\n",
        timestamp()
    );
    (subject, body)
}

#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    #[serde(rename = "@type")] pub type_: String,
    pub summary: String,
    #[serde(rename = "themeColor")] pub theme_color: String,
    pub title: String,
    pub text: String,
}

pub fn webhook_payload_dump(target: &str, machine: &str, dump_path: &str) -> WebhookPayload {
    WebhookPayload {
        type_: "MessageCard".into(),
        summary: format!("Dump created for {target}"),
        theme_color: "FF0000".into(),
        title: format!("[ProcDump] Dump created for {target} on {machine}"),
        text: format!(
            "**Target:** {target}\n\n**Computer:** {machine}\n\n**Dump File:** {dump_path}\n\n**Timestamp:** {}",
            timestamp()
        ),
    }
}

pub fn webhook_payload_warning(subject: &str, message: &str) -> WebhookPayload {
    WebhookPayload {
        type_: "MessageCard".into(),
        summary: subject.into(),
        theme_color: "FFAA00".into(),
        title: subject.into(),
        text: message.into(),
    }
}

/// Password seam: real DPAPI on windows (Task 7), passthrough elsewhere so
/// Linux tests never need DPAPI.
fn decrypt_password(cfg: &Config) -> String {
    #[cfg(windows)]
    { crate::secrets::unprotect(&cfg.encrypted_password_blob, crate::secrets::SMTP_ENTROPY) }
    #[cfg(not(windows))]
    { cfg.encrypted_password_blob.clone() }
}

fn effective_webhook_url(cfg: &Config) -> String {
    #[cfg(windows)]
    {
        if !cfg.encrypted_webhook_url_blob.is_empty() {
            return crate::secrets::unprotect(&cfg.encrypted_webhook_url_blob, crate::secrets::WEBHOOK_ENTROPY);
        }
    }
    cfg.webhook_url.clone()
}

pub fn send_email(cfg: &Config, subject: &str, body: &str) -> Result<(), String> {
    use lettre::message::Mailbox;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::transport::smtp::client::{Tls, TlsParameters};
    use lettre::{Message, SmtpTransport, Transport};

    let from: Mailbox = cfg.from_address.parse().map_err(|e| format!("From: {e}"))?;
    let mut msg = Message::builder().from(from).subject(subject);
    for to in split_addresses(&cfg.to_address) {
        msg = msg.to(to.parse().map_err(|e| format!("To '{to}': {e}"))?);
    }
    for cc in split_addresses(&cfg.cc_address) {
        msg = msg.cc(cc.parse().map_err(|e| format!("Cc '{cc}': {e}"))?);
    }
    let email = msg.body(body.to_string()).map_err(|e| e.to_string())?;

    let tls_params = TlsParameters::new(cfg.smtp_server.clone()).map_err(|e| e.to_string())?;
    let tls = match tls_mode(cfg.use_ssl, cfg.smtp_port) {
        TlsMode::Wrapper => Tls::Wrapper(tls_params),
        TlsMode::Required => Tls::Required(tls_params),
        TlsMode::Opportunistic => Tls::Opportunistic(tls_params),
    };
    let mut builder = SmtpTransport::builder_dangerous(&cfg.smtp_server)
        .port(cfg.smtp_port)
        .tls(tls)
        .timeout(Some(std::time::Duration::from_secs(30)));
    if !cfg.smtp_username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(
            cfg.smtp_username.clone(),
            decrypt_password(cfg),
        ));
    }
    builder.build().send(&email).map(|_| ()).map_err(|e| e.to_string())
}

pub fn send_test_email(cfg: &Config) -> Result<(), String> {
    let machine = machine_name();
    let subject = format!("[ProcDump] Test email from {machine}");
    let body = format!(
        "This is a test email from ProcDump Monitor.\r\n\r\n\
         Computer:   {machine}\r\n\
         Timestamp:  {}\r\n",
        timestamp()
    );
    send_email(cfg, &subject, &body)
}

/// Raw TCP connect + banner read (like Test-NetConnection). Does not send mail.
pub fn validate_smtp_connectivity(server: &str, port: u16, timeout_ms: u64) -> (bool, String) {
    use std::io::Read;
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let timeout = Duration::from_millis(timeout_ms);
    let addr = match format!("{server}:{port}").to_socket_addrs() {
        Ok(mut it) => match it.next() {
            Some(a) => a,
            None => return (false, format!("Cannot resolve {server}")),
        },
        Err(e) => return (false, format!("Cannot resolve {server}: {e}")),
    };
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(mut s) => {
            let _ = s.set_read_timeout(Some(timeout));
            let mut buf = [0u8; 1024];
            let banner = match s.read(&mut buf) {
                Ok(n) => String::from_utf8_lossy(&buf[..n]).trim().to_string(),
                Err(_) => String::new(),
            };
            (true, format!("Connected to {server}:{port}\r\nBanner: {banner}"))
        }
        Err(e) => (false, format!("Connection failed: {e}")),
    }
}

pub fn post_webhook(url: &str, payload: &WebhookPayload) {
    let result = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .post(url)
        .send_json(payload);
    match result {
        Ok(_) => logger::log("Webhook", "Webhook notification sent."),
        Err(e) => logger::log("Webhook", &format!("Webhook failed: {e}")),
    }
}

// ── Background queue: bounded, panic-isolated, never blocks the monitor ──

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct NotifyQueue {
    tx: std::sync::mpsc::SyncSender<Job>,
}

impl NotifyQueue {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(64);
        std::thread::spawn(move || {
            for job in rx {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            }
        });
        NotifyQueue { tx }
    }

    pub fn enqueue(&self, job: Job) {
        if self.tx.try_send(job).is_err() {
            logger::log("NotifyQ", "Notification queue full; dropping item.");
        }
    }

    pub fn enqueue_dump(&self, cfg: Config, dump_path: String) {
        if cfg.email_enabled {
            let c = cfg.clone();
            let p = dump_path.clone();
            self.enqueue(Box::new(move || {
                let (s, b) = dump_email(&c.target_name, &machine_name(), &p);
                match send_email(&c, &s, &b) {
                    Ok(()) => logger::log("NotifyQ", "Email: dump notification sent."),
                    Err(e) => logger::log("NotifyQ", &format!("Email failed: {e}")),
                }
            }));
        }
        if cfg.webhook_enabled {
            self.enqueue(Box::new(move || {
                let url = effective_webhook_url(&cfg);
                if !url.trim().is_empty() {
                    let payload = webhook_payload_dump(&cfg.target_name, &machine_name(), &dump_path);
                    post_webhook(&url, &payload);
                }
            }));
        }
    }

    pub fn enqueue_warning(&self, cfg: Config, subject: String, message: String) {
        if cfg.email_enabled {
            let c = cfg.clone();
            let (s2, m2) = (subject.clone(), message.clone());
            self.enqueue(Box::new(move || {
                if let Err(e) = send_email(&c, &s2, &m2) {
                    logger::log("NotifyQ", &format!("Warning email failed: {e}"));
                }
            }));
        }
        if cfg.webhook_enabled {
            self.enqueue(Box::new(move || {
                let url = effective_webhook_url(&cfg);
                if !url.trim().is_empty() {
                    post_webhook(&url, &webhook_payload_warning(&subject, &message));
                }
            }));
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cd rust && cargo test notify 2>&1 | tail -5`
Expected: `test result: ok. 5 passed`

- [ ] **Step 5: Commit**

```bash
git add rust/src/notify.rs rust/src/main.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: email + webhook notifiers with background queue"
```

---

### Task 7: secrets (DPAPI) + bitness resolver + services parser

**Files:**
- Create: `rust/src/secrets.rs`, `rust/src/bitness.rs`, `rust/src/services.rs`
- Modify: `rust/src/main.rs` (mod declarations)

**Interfaces:**
- Produces:
  `secrets::SMTP_ENTROPY: &[u8]` (= `b"ProcDumpMonitor-SMTP-v1"`), `secrets::WEBHOOK_ENTROPY: &[u8]` (= `b"ProcDumpMonitor-Webhook-v1"`), `secrets::protect(plain: &str, entropy: &[u8]) -> String` (base64, "" for empty input), `secrets::unprotect(b64: &str, entropy: &[u8]) -> String` ("" on any failure) — both `#[cfg(windows)]`;
  `bitness::Bitness { Unknown, X86, X64 }`, `bitness::select_binary(bitness: Bitness, procdump_dir: &Path, os_is_64: bool) -> BinaryChoice` (pure) with `pub struct BinaryChoice { pub actual: PathBuf, pub warning: Option<String>, pub summary: String }` (empty `actual` = none found), `bitness::detect(process_name: &str) -> Bitness` (`#[cfg(windows)]`);
  `services::ServiceInfo { pub name: String, pub display: String, pub running: bool }`, `services::parse_sc_output(&str) -> Vec<ServiceInfo>` (pure), `services::list() -> Vec<ServiceInfo>` (`#[cfg(windows)]`, runs `sc.exe query type= service state= all`).

- [ ] **Step 1: Write failing tests** (pure parts, Linux)

`bitness.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[&str]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pdm_bit_{}", files.join("_").len()));
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
}
```

`services.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SC_SAMPLE: &str = "\
SERVICE_NAME: Spooler\r
DISPLAY_NAME: Print Spooler\r
        TYPE               : 110  WIN32_OWN_PROCESS (interactive)\r
        STATE              : 4  RUNNING\r
                                (STOPPABLE, NOT_PAUSABLE, ACCEPTS_SHUTDOWN)\r
\r
SERVICE_NAME: Fax\r
DISPLAY_NAME: Fax\r
        TYPE               : 10  WIN32_OWN_PROCESS\r
        STATE              : 1  STOPPED\r
\r
";

    #[test]
    fn parses_name_display_state() {
        let svcs = parse_sc_output(SC_SAMPLE);
        assert_eq!(svcs.len(), 2);
        assert_eq!(svcs[0].name, "Spooler");
        assert_eq!(svcs[0].display, "Print Spooler");
        assert!(svcs[0].running);
        assert_eq!(svcs[1].name, "Fax");
        assert!(!svcs[1].running);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test 'bitness|services' 2>&1 | tail -5` — expected: compile errors.

- [ ] **Step 3: Implement**

`rust/src/secrets.rs` (whole file `#![cfg(windows)]` via `#[cfg(windows)] mod` gating in main.rs — simplest: wrap contents):

```rust
#![allow(unsafe_code)]
// Windows-only: DPAPI LocalMachine so blobs decrypt under SYSTEM.
#[cfg(windows)]
pub use imp::*;

pub const SMTP_ENTROPY: &[u8] = b"ProcDumpMonitor-SMTP-v1";
pub const WEBHOOK_ENTROPY: &[u8] = b"ProcDumpMonitor-Webhook-v1";

#[cfg(windows)]
mod imp {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
        CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN,
    };

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    /// Encrypt with DPAPI LocalMachine; returns base64. Empty input -> "".
    pub fn protect(plain: &str, entropy: &[u8]) -> String {
        if plain.is_empty() { return String::new(); }
        let input = blob(plain.as_bytes());
        let ent = blob(entropy);
        let mut out = CRYPT_INTEGER_BLOB::default();
        let ok = unsafe {
            CryptProtectData(&input, None, Some(&ent), None, None,
                CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        if ok.is_err() { return String::new(); }
        let bytes = unsafe {
            std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec()
        };
        unsafe { LocalFree(HLOCAL(out.pbData as *mut core::ffi::c_void)); }
        B64.encode(bytes)
    }

    /// Decrypt a base64 DPAPI blob; "" on any failure (matches C#).
    pub fn unprotect(b64: &str, entropy: &[u8]) -> String {
        if b64.is_empty() { return String::new(); }
        let Ok(encrypted) = B64.decode(b64) else { return String::new(); };
        let input = blob(&encrypted);
        let ent = blob(entropy);
        let mut out = CRYPT_INTEGER_BLOB::default();
        let ok = unsafe {
            CryptUnprotectData(&input, None, Some(&ent), None, None,
                CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        if ok.is_err() { return String::new(); }
        let bytes = unsafe {
            std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec()
        };
        unsafe { LocalFree(HLOCAL(out.pbData as *mut core::ffi::c_void)); }
        String::from_utf8(bytes).unwrap_or_default()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn roundtrip_and_wrong_entropy_fails() {
            let blob = protect("hunter2", super::super::SMTP_ENTROPY);
            assert!(!blob.is_empty());
            assert_eq!(unprotect(&blob, super::super::SMTP_ENTROPY), "hunter2");
            assert_eq!(unprotect(&blob, b"wrong-entropy"), "");
            assert_eq!(unprotect("not-base64!!!", super::super::SMTP_ENTROPY), "");
        }
    }
}
```

(The DPAPI test runs only in `scripts/vm-build.sh test` on the VM.)

`rust/src/bitness.rs`:

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bitness { Unknown, X86, X64 }

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

/// Find a PID by exe name (case-insensitive, .exe optional) via Toolhelp,
/// then classify with IsWow64Process2 resolved via GetProcAddress.
/// CRITICAL: IsWow64Process2 does not exist on Server 2016 (build 14393) —
/// a static windows-crate import would make the exe FAIL TO LOAD there.
#[cfg(windows)]
pub fn detect(process_name: &str) -> Bitness {
    use windows::core::{s, PCSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
        PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows::Win32::System::Threading::{
        IsWow64Process, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
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
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows::Win32::System::Threading::IsWow64Process;
    use windows::Win32::Foundation::BOOL;

    const IMAGE_FILE_MACHINE_I386: u16 = 0x014C;
    const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
    const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;

    // Dynamic lookup — Win10 1709+ only, absent on Server 2016.
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
```

`rust/src/services.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub display: String,
    pub running: bool,
}

/// Parse `sc.exe query type= service state= all` output. Locale caveat:
/// SERVICE_NAME/DISPLAY_NAME/STATE tokens are not localized (verified en-US;
/// C-CURE deployments are en-US).
pub fn parse_sc_output(out: &str) -> Vec<ServiceInfo> {
    let mut result = Vec::new();
    let mut cur: Option<ServiceInfo> = None;
    for line in out.lines() {
        let line = line.trim_end();
        if let Some(name) = line.strip_prefix("SERVICE_NAME: ") {
            if let Some(svc) = cur.take() { result.push(svc); }
            cur = Some(ServiceInfo { name: name.trim().into(), display: String::new(), running: false });
        } else if let Some(disp) = line.trim_start().strip_prefix("DISPLAY_NAME: ") {
            if let Some(svc) = cur.as_mut() { svc.display = disp.trim().into(); }
        } else if line.trim_start().starts_with("STATE") {
            if let Some(svc) = cur.as_mut() {
                svc.running = line.contains(" 4 ") || line.ends_with("RUNNING");
            }
        }
    }
    if let Some(svc) = cur { result.push(svc); }
    result
}

#[cfg(windows)]
pub fn list() -> Vec<ServiceInfo> {
    std::process::Command::new("sc.exe")
        .args(["query", "type=", "service", "state=", "all"])
        .output()
        .map(|o| parse_sc_output(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}
```

Add to `main.rs`: `mod secrets; mod bitness; mod services;`

- [ ] **Step 4: Run Linux tests, then full VM test suite**

Run: `cd rust && cargo test 2>&1 | tail -5`
Expected: all green including new bitness (6) + services (1).

Run: `scripts/vm-build.sh test`
Expected: `CARGO_EXIT=0` with the DPAPI roundtrip test passing (`secrets::imp::tests::roundtrip_and_wrong_entropy_fails ... ok`). This is the first VM test run — it validates the whole windows-gated codebase compiles and passes.

- [ ] **Step 5: Commit**

```bash
git add rust/src/{secrets,bitness,services}.rs rust/src/main.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: DPAPI secrets, bitness resolver (GetProcAddress-safe), sc parser"
```

---

### Task 8: CLI verbs + monitor loop + VM integration test

**Files:**
- Create: `rust/src/cli.rs`, `rust/src/monitor.rs`
- Modify: `rust/src/main.rs` (replace spike main with real dispatch)

**Interfaces:**
- Consumes: everything from Tasks 2–7.
- Produces: `cli::Verb` enum + `cli::parse(args: &[String]) -> Result<Verb, String>` (pure, Linux-tested), `cli::run(verb: Verb) -> i32` (windows), `monitor::run(cfg: Config) -> ()` (windows, infinite loop).
- Exit codes: 0 success, 1 failure, 2 bad args.
- Verbs (leading dashes optional — `--monitor` and `monitor` both work):
  `monitor`, `install`, `uninstall`, `start`, `stop`, `status`, `version`, `help`; all accept `--config <path>` (default `paths::config_path()`).

- [ ] **Step 1: Write failing parser tests** (in `cli.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

    #[test]
    fn parses_verbs_with_and_without_dashes() {
        assert!(matches!(parse(&s(&["--monitor"])).unwrap(), Verb::Monitor { .. }));
        assert!(matches!(parse(&s(&["install"])).unwrap(), Verb::Install { .. }));
        assert!(matches!(parse(&s(&["--status"])).unwrap(), Verb::Status { .. }));
        assert!(matches!(parse(&s(&["--version"])).unwrap(), Verb::Version));
        assert!(matches!(parse(&s(&["help"])).unwrap(), Verb::Help));
    }

    #[test]
    fn config_override() {
        let Verb::Monitor { config } = parse(&s(&["--monitor", "--config", r"C:\x\c.json"])).unwrap()
            else { panic!() };
        assert_eq!(config, std::path::PathBuf::from(r"C:\x\c.json"));
    }

    #[test]
    fn bad_verb_and_missing_config_value_error() {
        assert!(parse(&s(&["--frobnicate"])).is_err());
        assert!(parse(&s(&["install", "--config"])).is_err());
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cd rust && cargo test cli 2>&1 | tail -3` → compile error.

- [ ] **Step 3: Implement `cli.rs`**

```rust
use std::path::PathBuf;

#[derive(Debug)]
pub enum Verb {
    Monitor { config: PathBuf },
    Install { config: PathBuf },
    Uninstall { config: PathBuf },
    Start { config: PathBuf },
    Stop { config: PathBuf },
    Status { config: PathBuf },
    Version,
    Help,
}

pub const USAGE: &str = "\
ProcDumpMonitor.exe                     launch the GUI wizard
ProcDumpMonitor.exe <verb> [--config <path>]
  verbs: monitor | install | uninstall | start | stop | status | version | help
  exit codes: 0 = success, 1 = failure, 2 = bad arguments";

pub fn parse(args: &[String]) -> Result<Verb, String> {
    let verb = args[0].trim_start_matches('-').to_ascii_lowercase();
    let mut config = crate::paths::config_path();
    let mut i = 1;
    while i < args.len() {
        match args[i].trim_start_matches('-') {
            "config" => {
                i += 1;
                let v = args.get(i).ok_or("--config requires a path")?;
                config = PathBuf::from(v);
            }
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }
    Ok(match verb.as_str() {
        "monitor" => Verb::Monitor { config },
        "install" => Verb::Install { config },
        "uninstall" => Verb::Uninstall { config },
        "start" => Verb::Start { config },
        "stop" => Verb::Stop { config },
        "status" => Verb::Status { config },
        "version" => Verb::Version,
        "help" => Verb::Help,
        other => return Err(format!("unknown verb: {other}")),
    })
}

#[cfg(windows)]
pub fn run(verb: Verb) -> i32 {
    use crate::{config::Config, logger, monitor, paths, task};

    fn load_and_init(config_path: &std::path::Path) -> Config {
        let cfg = Config::load(config_path);
        logger::init(paths::log_path(), cfg.max_log_size_mb, cfg.max_log_files);
        cfg
    }

    fn report(res: Result<(), String>, ok_msg: &str) -> i32 {
        match res {
            Ok(()) => { println!("{ok_msg}"); 0 }
            Err(e) => { eprintln!("ERROR: {e}"); 1 }
        }
    }

    match verb {
        Verb::Version => { println!("{}", env!("CARGO_PKG_VERSION")); 0 }
        Verb::Help => { println!("{USAGE}"); 0 }
        Verb::Monitor { config } => {
            let cfg = load_and_init(&config);
            monitor::run(cfg);
            0
        }
        Verb::Install { config } => {
            let cfg = load_and_init(&config);
            match task::install(&cfg) {
                Ok(existed) => {
                    println!("Task '{}' {}.", task::sanitize_task_name(&cfg.task_name),
                             if existed { "updated" } else { "created" });
                    0
                }
                Err(e) => { eprintln!("ERROR: {e}"); 1 }
            }
        }
        Verb::Uninstall { config } => {
            let cfg = load_and_init(&config);
            report(task::uninstall(&task::sanitize_task_name(&cfg.task_name)), "Task removed.")
        }
        Verb::Start { config } => {
            let cfg = load_and_init(&config);
            report(task::start(&task::sanitize_task_name(&cfg.task_name)), "Task started.")
        }
        Verb::Stop { config } => {
            let cfg = load_and_init(&config);
            report(task::stop(&task::sanitize_task_name(&cfg.task_name)), "Task stopped.")
        }
        Verb::Status { config } => {
            let cfg = load_and_init(&config);
            let st = task::query_status(&task::sanitize_task_name(&cfg.task_name));
            println!("{}", serde_json::to_string_pretty(&st).unwrap_or_default());
            0
        }
    }
}
```

- [ ] **Step 4: Implement `monitor.rs`** (windows-only; port of `ProcDumpMonitorLoop`)

```rust
#![cfg(windows)]
use crate::config::Config;
use crate::notify::NotifyQueue;
use crate::{bitness, diskguard, health, logger, paths, procdump, retention, stability};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

static STOPPING: AtomicBool = AtomicBool::new(false);

fn install_ctrl_c_handler() {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    unsafe extern "system" fn handler(_: u32) -> BOOL {
        STOPPING.store(true, Ordering::SeqCst);
        BOOL(1)
    }
    unsafe { let _ = SetConsoleCtrlHandler(Some(handler), true); }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

pub fn run(mut cfg: Config) {
    STOPPING.store(false, Ordering::SeqCst);
    install_ctrl_c_handler();

    let health_path = paths::health_path();
    let mut h = health::load(&health_path); // resume TotalDumpCount across restarts
    h.monitor_pid = std::process::id();
    h.version = env!("CARGO_PKG_VERSION").into();

    logger::log("Monitor", "ProcDump Monitor started.");
    logger::log("Monitor", &format!("Target: {} ({:?})", cfg.target_name, cfg.target_type));

    // Bitness-based binary switch (non-fatal on failure)
    let pd_dir = Path::new(&cfg.proc_dump_path).parent()
        .map(|p| p.to_path_buf()).unwrap_or_else(paths::install_dir);
    let os_is_64 = std::env::var("PROCESSOR_ARCHITECTURE").map(|a| a != "x86").unwrap_or(true)
        || std::env::var("PROCESSOR_ARCHITEW6432").is_ok();
    let choice = bitness::select_binary(bitness::detect(&cfg.target_name), &pd_dir, os_is_64);
    logger::log("Monitor", &format!("Bitness: {}", choice.summary));
    if let Some(w) = &choice.warning { logger::log("Monitor", &format!("Bitness WARNING: {w}")); }
    if choice.actual.exists() && choice.actual != Path::new(&cfg.proc_dump_path) {
        logger::log("Monitor", &format!("Switching ProcDump binary -> {}", choice.actual.display()));
        cfg.proc_dump_path = choice.actual.display().to_string();
    }

    logger::log("Monitor", &format!("ProcDump args: {}", procdump::build_args(&cfg)));

    if std::fs::create_dir_all(&cfg.dump_directory).is_err() {
        logger::log("Monitor", "Cannot create dump directory - exiting.");
        return;
    }

    let queue = NotifyQueue::new();
    let mut last_low_disk_notify: Option<Instant> = None;

    while !STOPPING.load(Ordering::SeqCst) {
        let cycle_start = SystemTime::now();
        h.last_cycle_utc = now_iso();
        h.last_error.clear();
        h.disk_space_low = false;
        logger::log("Monitor", "-- Cycle start --");

        // Disk guard
        let mut skip_cycle = false;
        if cfg.min_free_disk_mb > 0 {
            let (ok, free_mb) = diskguard::check_free_space(Path::new(&cfg.dump_directory), cfg.min_free_disk_mb);
            h.free_disk_mb = free_mb;
            h.disk_space_low = !ok;
            if !ok {
                let warn = format!("Skipping cycle -- only {free_mb} MB free (threshold: {} MB)", cfg.min_free_disk_mb);
                logger::log("Monitor", &warn);
                // rate-limited to once per hour
                if last_low_disk_notify.map_or(true, |t| t.elapsed() >= Duration::from_secs(3600)) {
                    last_low_disk_notify = Some(Instant::now());
                    queue.enqueue_warning(cfg.clone(),
                        format!("[ProcDump] Low disk warning on {}", crate::notify::machine_name()),
                        warn);
                }
                skip_cycle = true;
            }
        }

        if !skip_cycle {
            retention::apply(Path::new(&cfg.dump_directory), cfg.dump_retention_days, cfg.dump_retention_max_gb);
            if let Err(e) = run_procdump_cycle(&cfg, cycle_start, &queue, &mut h, &health_path) {
                h.last_error = e.clone();
                logger::log("Monitor", &format!("Cycle error: {e}"));
            }
        }

        h.next_retry_utc = now_iso();
        health::write(&health_path, &h);

        // interruptible sleep
        let delay = cfg.restart_delay_seconds.max(0) as u64 * 10;
        for _ in 0..delay {
            if STOPPING.load(Ordering::SeqCst) { break; }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    logger::log("Monitor", "ProcDump Monitor stopped.");
}

fn run_procdump_cycle(
    cfg: &Config,
    cycle_start: SystemTime,
    queue: &NotifyQueue,
    h: &mut health::HealthStatus,
    health_path: &Path,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut child = std::process::Command::new(&cfg.proc_dump_path)
        .raw_arg(procdump::build_args(cfg))
        .current_dir(&cfg.dump_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("cannot launch procdump: {e}"))?;

    h.proc_dump_pid = child.id();

    // stream output to the log from reader threads
    let spawn_reader = |stream: Option<Box<dyn std::io::Read + Send>>, tag: &'static str| {
        if let Some(s) = stream {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    logger::log(tag, &line);
                }
            });
        }
    };
    spawn_reader(child.stdout.take().map(|s| Box::new(s) as _), "ProcDump");
    spawn_reader(child.stderr.take().map(|s| Box::new(s) as _), "ProcDump-ERR");

    // wait with 30s health heartbeat so "waiting for target" != "stalled"
    let mut beats = 0u32;
    let exit_code = loop {
        if STOPPING.load(Ordering::SeqCst) {
            let _ = child.kill();
            break -1;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) => {
                std::thread::sleep(Duration::from_secs(1));
                beats += 1;
                if beats % 30 == 0 {
                    h.last_cycle_utc = now_iso();
                    health::write(health_path, h);
                }
            }
            Err(e) => return Err(format!("wait failed: {e}")),
        }
    };

    h.proc_dump_pid = 0;
    h.last_proc_dump_exit_code = exit_code;
    logger::log("Monitor", &format!("ProcDump exited with code {exit_code}."));

    detect_and_notify(cfg, cycle_start, queue, h);
    Ok(())
}

fn detect_and_notify(cfg: &Config, cycle_start: SystemTime, queue: &NotifyQueue, h: &mut health::HealthStatus) {
    let Ok(rd) = std::fs::read_dir(&cfg.dump_directory) else { return };
    let newest = rd.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("dmp")))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            let t = m.modified().ok()?;
            (t >= cycle_start).then_some((e.path(), t))
        })
        .max_by_key(|(_, t)| *t);

    let Some((path, _)) = newest else {
        logger::log("Monitor", "No new dump file detected in this cycle.");
        return;
    };

    logger::log("Monitor", &format!("New dump detected: {}. Checking stability...", path.display()));
    if !stability::wait_for_stable_file(&path, cfg.dump_stability_timeout_seconds, cfg.dump_stability_poll_seconds) {
        h.last_error = "Dump file still locked after timeout - notification suppressed.".into();
        return;
    }

    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    h.last_dump_file_name = file_name.clone();
    h.total_dump_count += 1;

    if h.last_notified_dump_file == file_name {
        logger::log("Monitor", "Dump already notified - skipping duplicate notification.");
        return;
    }

    queue.enqueue_dump(cfg.clone(), path.display().to_string());
    h.last_notified_dump_file = file_name;
    h.last_notified_utc = now_iso();
}
```

- [ ] **Step 5: Replace `main.rs` with real dispatch**

```rust
#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod bitness;
mod cli;
mod config;
mod diskguard;
mod health;
mod logger;
#[cfg(windows)]
mod monitor;
mod notify;
mod paths;
mod procdump;
mod retention;
mod secrets;
mod services;
mod stability;
mod task;
#[cfg(windows)]
mod gui; // created in Task 9; until then keep this line commented out

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        gui::run(); // Task 9; until then: `return;` with a TODO-free comment
        return;
    }
    attach_console();
    let code = match cli::parse(&args) {
        Ok(verb) => cli::run(verb),
        Err(e) => {
            eprintln!("ERROR: {e}\n{}", cli::USAGE);
            2
        }
    };
    std::process::exit(code);
}

/// windows_subsystem = "windows" detaches stdio; reattach to the parent
/// console so CLI verbs print. No-op when launched by Task Scheduler.
#[cfg(windows)]
fn attach_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe { let _ = AttachConsole(ATTACH_PARENT_PROCESS); }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("ProcDumpMonitor targets Windows; Linux builds are for `cargo test` only.");
}
```

Until Task 9 exists, `mod gui;` and `gui::run()` won't compile — use `eprintln!("GUI arrives in Task 9"); return;` in the no-args branch and leave the mod line commented. Task 9 uncomments both.

- [ ] **Step 6: Linux tests green, VM build green**

Run: `cd rust && cargo test 2>&1 | tail -3` → all pass.
Run: `scripts/vm-build.sh build` → `dist/ProcDumpMonitor.exe` fetched.

- [ ] **Step 7: VM INTEGRATION TEST — the whole headless product, end to end**

Download procdump on the VM and stage an install dir:

```bash
scripts/vm.sh '
if (!(Test-Path C:\PDMTest)) { mkdir C:\PDMTest | Out-Null }
if (!(Test-Path C:\PDMTest\procdump64.exe)) {
  Invoke-WebRequest -Uri https://download.sysinternals.com/files/Procdump.zip -OutFile C:\PDMTest\pd.zip
  Expand-Archive C:\PDMTest\pd.zip -DestinationPath C:\PDMTest -Force
}
Copy-Item C:\pdm\target\release\ProcDumpMonitor.exe C:\PDMTest\ -Force
"staged"'
```

Write a test config targeting notepad, exercise every verb, then a real monitor cycle:

```bash
scripts/vm.sh '
$cfg = @{
  ConfigVersion = 3; TargetName = "notepad"; TargetType = "Process"
  ProcDumpPath = "C:\PDMTest\procdump64.exe"; DumpDirectory = "C:\PDMTest\Dumps"
  DumpType = "Mini"; DumpOnException = $false; DumpOnTerminate = $true
  UseClone = $false; MaxDumps = 1; RestartDelaySeconds = 5
  Scenario = "Custom"; WaitForProcess = $true
  MinFreeDiskMB = 100; DumpStabilityTimeoutSeconds = 30; DumpStabilityPollSeconds = 2
  MaxLogSizeMB = 10; MaxLogFiles = 5
  TaskName = "PDM Rust Test"
} | ConvertTo-Json
Set-Content C:\PDMTest\config.json $cfg
cd C:\PDMTest
.\ProcDumpMonitor.exe version
.\ProcDumpMonitor.exe install
"INSTALL_EXIT=$LASTEXITCODE"
schtasks /Query /TN "PDM Rust Test" /V /FO LIST | Select-String "Run As User|Schedule Type"
.\ProcDumpMonitor.exe status
.\ProcDumpMonitor.exe start
"START_EXIT=$LASTEXITCODE"
Start-Sleep 3
# monitor is now waiting (-w) for notepad: start it, then kill it -> -t dump
Start-Process notepad
Start-Sleep 5
Stop-Process -Name notepad -Force
Start-Sleep 20
Get-ChildItem C:\PDMTest\Dumps
Get-Content C:\PDMTest\health.json
.\ProcDumpMonitor.exe stop
.\ProcDumpMonitor.exe uninstall
"UNINSTALL_EXIT=$LASTEXITCODE"'
```

Expected: version prints; `INSTALL_EXIT=0`; query shows `Run As User: SYSTEM` + `At system start up`; status JSON has `"Exists": true`; a `notepad*.dmp` file exists; `health.json` shows `"TotalDumpCount": 1` and the dump in `"LastNotifiedDumpFile"`; `UNINSTALL_EXIT=0`. Also verify a log file exists: `scripts/vm.sh 'Get-Content C:\PDMTest\Logs\procdump.log -Tail 20'`.

- [ ] **Step 8: Commit**

```bash
git add rust/src/cli.rs rust/src/monitor.rs rust/src/main.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: CLI verbs + monitor loop; VM e2e passes (SYSTEM task, dump, health)"
```

---

### Task 9: GUI shell + Target page

GUI tasks are verified by building on the VM and driving the window over RDP/manually — ask the user to eyeball each page when it lands. Automated assertion where possible: exe still passes `scripts/vm-build.sh test`.

**Files:**
- Create: `rust/src/gui/mod.rs`, `rust/src/gui/page_target.rs`
- Modify: `rust/src/main.rs` (uncomment `mod gui;` + `gui::run()`)

**Interfaces:**
- Consumes: `config::Config`, `services::{list, ServiceInfo}`, `task::auto_task_name`.
- Produces: `gui::run()` — builds the wizard window, loads `Config` from `paths::config_path()`, dispatches nwg events; `gui::WizardState { pub cfg: RefCell<Config>, pub dirty_scenario: Cell<bool> }` shared via `Rc`.
- Page contract (every page implements): `fn build(parent: &nwg::Window, state: Rc<WizardState>) -> PageControls`, `fn load(&self, cfg: &Config)` (config → controls), `fn save(&self, cfg: &mut Config)` (controls → config, called on page-leave).

**Wizard shell spec:**
- Window: 780×580, fixed (no resize), title `ProcDump Monitor`, icon from embedded resource.
- Top: full-width label `Step {n} of 6 — {Target|ProcDump|Task|Notify|Review|About}` in bold.
- Middle: one `nwg::Frame` per page (same rect: 10,40 → 760,480), only current visible.
- Bottom: `← Back` (10,530 90×30), `Next →` (680,530 90×30). Back disabled on page 1; Next relabels to nothing special on page 6 (About is last, Next disabled). Page switch = `save()` current page → `load()` next page → toggle frame visibility → update step label.
- Close (X) = exit without implicit save (Review page has explicit save buttons).

- [ ] **Step 1: Implement shell** — `rust/src/gui/mod.rs`:

```rust
#![cfg(windows)]
mod page_target;
// Task 10 adds: mod page_procdump; mod page_task;
// Task 11 adds: mod page_notify; mod page_review; mod page_about;

use crate::config::Config;
use crate::paths;
use native_windows_gui as nwg;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct WizardState {
    pub cfg: RefCell<Config>,
}

const STEP_TITLES: [&str; 6] = ["Target", "ProcDump", "Task", "Notify", "Review", "About"];

pub fn run() {
    nwg::init().expect("nwg init failed");
    let _ = nwg::Font::set_global_family("Segoe UI");

    let state = Rc::new(WizardState {
        cfg: RefCell::new(Config::load(&paths::config_path())),
    });

    let mut window = nwg::Window::default();
    nwg::Window::builder()
        .size((780, 580))
        .center(true)
        .title("ProcDump Monitor")
        .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
        .build(&mut window)
        .expect("window");

    let mut step_label = nwg::Label::default();
    nwg::Label::builder()
        .text("Step 1 of 6 - Target")
        .position((10, 8))
        .size((600, 24))
        .parent(&window)
        .build(&mut step_label)
        .expect("step label");

    let mut back_btn = nwg::Button::default();
    nwg::Button::builder().text("< Back").position((10, 530)).size((90, 30))
        .enabled(false).parent(&window).build(&mut back_btn).expect("back");
    let mut next_btn = nwg::Button::default();
    nwg::Button::builder().text("Next >").position((680, 530)).size((90, 30))
        .parent(&window).build(&mut next_btn).expect("next");

    // One frame per page, identical rect; pages build their controls inside.
    let mut frames: Vec<nwg::Frame> = Vec::new();
    for i in 0..6 {
        let mut f = nwg::Frame::default();
        nwg::Frame::builder()
            .position((10, 40))
            .size((760, 480))
            .flags(if i == 0 { nwg::FrameFlags::VISIBLE } else { nwg::FrameFlags::NONE })
            .parent(&window)
            .build(&mut f)
            .expect("frame");
        frames.push(f);
    }

    let target_page = Rc::new(page_target::build(&frames[0], state.clone()));
    target_page.load(&state.cfg.borrow());
    // Task 10/11: build remaining pages the same way.

    let current = Rc::new(Cell::new(0usize));
    let window_handle = window.handle;

    let handler = {
        let state = state.clone();
        let current = current.clone();
        let target_page = target_page.clone();
        let frames_h: Vec<nwg::ControlHandle> = frames.iter().map(|f| f.handle).collect();
        let back_h = back_btn.handle;
        let next_h = next_btn.handle;
        let step_h = step_label.handle;
        nwg::full_bind_event_handler(&window_handle, move |evt, _data, handle| {
            match evt {
                nwg::Event::OnWindowClose if handle == window_handle => nwg::stop_thread_dispatch(),
                nwg::Event::OnButtonClick if handle == back_h || handle == next_h => {
                    let cur = current.get();
                    let next = if handle == next_h { cur + 1 } else { cur.saturating_sub(1) };
                    if next >= 6 { return; }
                    // save current page (only pages that exist so far)
                    if cur == 0 { target_page.save(&mut state.cfg.borrow_mut()); }
                    // Task 10/11: save/load arms for pages 1..=5
                    unsafe {
                        use nwg::win32::window_helper as wh;
                        wh::set_window_visibility(frames_h[cur].hwnd().unwrap(), false);
                        wh::set_window_visibility(frames_h[next].hwnd().unwrap(), true);
                    }
                    if next == 0 { target_page.load(&state.cfg.borrow()); }
                    nwg::ControlHandle::from(step_h)
                        .set_text(&format!("Step {} of 6 - {}", next + 1, STEP_TITLES[next]));
                    // enable/disable nav
                    // (Back enabled iff next > 0; Next enabled iff next < 5)
                    current.set(next);
                }
                _ => {}
            }
        })
    };

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
}
```

Implementation note for the executor: nwg's plain-handle `set_text`/visibility helpers differ by version — if `nwg::win32::window_helper` isn't public in 1.0.13, keep `Rc<nwg::Frame>`/`Rc<nwg::Label>` clones in the closure instead of raw handles and call `.set_visible(bool)` / `.set_text(..)` on them (both exist on the control structs). Prefer the control-struct route; the handle route above is the fallback.

- [ ] **Step 2: Implement Target page** — `rust/src/gui/page_target.rs`:

Controls (all inside the page frame; label column x=10, control column x=170, width 380 unless noted):

| Control | nwg type | Pos (x,y) | Purpose |
|---|---|---|---|
| `lbl_process` "Process Name (no .exe):" | Label | 10,20 | |
| `txt_process` | TextInput | 170,18 | `cfg.target_name`; typing sets `TargetType::Process` |
| `lbl_service` "Select Service:" | Label | 10,60 | |
| `cmb_service` | ComboBox\<String\> | 170,58 | picking sets `target_name` = internal name, `TargetType::Service` |
| `chk_show_all` "Show all services" | CheckBox | 170,95 | unchecked = Running only |
| `btn_refresh` "Refresh Services" | Button (120×26) | 320,92 | re-enumerate |
| `lbl_hint` | Label (740 wide, gray) | 10,140 | "Picking a service fills the name and targets it as a service; typing targets a process." |

```rust
use crate::config::{Config, TargetType};
use crate::services;
use native_windows_gui as nwg;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TargetPage {
    pub txt_process: nwg::TextInput,
    pub cmb_service: nwg::ComboBox<String>,
    pub chk_show_all: nwg::CheckBox,
    pub btn_refresh: nwg::Button,
    /// (internal_name, running) parallel to combo items
    pub services: RefCell<Vec<services::ServiceInfo>>,
    pub picked_service: RefCell<Option<String>>,
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> TargetPage {
    let mut lbl = nwg::Label::default();
    nwg::Label::builder().text("Process Name (no .exe):").position((10, 20)).size((150, 22))
        .parent(parent).build(&mut lbl).unwrap();
    let mut txt_process = nwg::TextInput::default();
    nwg::TextInput::builder().position((170, 18)).size((380, 24))
        .parent(parent).build(&mut txt_process).unwrap();

    let mut lbl2 = nwg::Label::default();
    nwg::Label::builder().text("Select Service:").position((10, 60)).size((150, 22))
        .parent(parent).build(&mut lbl2).unwrap();
    let mut cmb_service = nwg::ComboBox::default();
    nwg::ComboBox::builder().position((170, 58)).size((380, 26))
        .parent(parent).build(&mut cmb_service).unwrap();

    let mut chk_show_all = nwg::CheckBox::default();
    nwg::CheckBox::builder().text("Show all services").position((170, 95)).size((140, 24))
        .parent(parent).build(&mut chk_show_all).unwrap();
    let mut btn_refresh = nwg::Button::default();
    nwg::Button::builder().text("Refresh Services").position((320, 92)).size((120, 26))
        .parent(parent).build(&mut btn_refresh).unwrap();

    let mut hint = nwg::Label::default();
    nwg::Label::builder()
        .text("Picking a service fills the name and targets it as a service; typing targets a process.")
        .position((10, 140)).size((740, 22)).parent(parent).build(&mut hint).unwrap();

    let page = TargetPage {
        txt_process, cmb_service, chk_show_all, btn_refresh,
        services: RefCell::new(Vec::new()),
        picked_service: RefCell::new(None),
    };
    page.refresh_services();
    page
}

impl TargetPage {
    pub fn refresh_services(&self) {
        let all = services::list();
        let show_all = self.chk_show_all.check_state() == nwg::CheckBoxState::Checked;
        let filtered: Vec<services::ServiceInfo> =
            all.into_iter().filter(|s| show_all || s.running).collect();
        self.cmb_service.clear();
        for s in &filtered {
            self.cmb_service.push(format!("{} ({})", s.display, s.name));
        }
        *self.services.borrow_mut() = filtered;
    }

    /// Wire in gui::run's event handler:
    /// - OnComboxBoxSelection on cmb_service -> on_service_picked()
    /// - OnButtonClick on btn_refresh, and OnButtonClick on chk_show_all -> refresh_services()
    pub fn on_service_picked(&self) {
        if let Some(i) = self.cmb_service.selection() {
            if let Some(svc) = self.services.borrow().get(i) {
                self.txt_process.set_text(&svc.name);
                *self.picked_service.borrow_mut() = Some(svc.name.clone());
            }
        }
    }

    pub fn load(&self, cfg: &Config) {
        self.txt_process.set_text(&cfg.target_name);
        if cfg.target_type == TargetType::Service {
            *self.picked_service.borrow_mut() = Some(cfg.target_name.clone());
        }
    }

    pub fn save(&self, cfg: &mut Config) {
        let typed = self.txt_process.text().trim().to_string();
        // If the text still equals the last-picked service name -> Service, else Process
        let picked = self.picked_service.borrow();
        cfg.target_type = match picked.as_deref() {
            Some(name) if name.eq_ignore_ascii_case(&typed) => TargetType::Service,
            _ => TargetType::Process,
        };
        // Auto task name follows target when the user hasn't customized it (Task 10's page re-checks)
        if cfg.task_name == crate::task::auto_task_name(&cfg.target_name)
            || cfg.task_name == "ProcDump Monitor" {
            cfg.task_name = crate::task::auto_task_name(&typed);
        }
        cfg.target_name = typed;
    }
}
```

Wire the three events listed in the doc comment into the shell's `full_bind_event_handler` (match on `handle == page.btn_refresh.handle`, etc.).

- [ ] **Step 3: Build on VM and manually verify**

Run: `scripts/vm-build.sh build`, then launch on the VM (`scripts/vm.sh 'Start-Process C:\pdm\target\release\ProcDumpMonitor.exe'`).
**Checkpoint for the user (RDP to 192.168.69.110, pw <redacted-rotate-me>):** window opens, service dropdown lists running services, picking one fills the textbox, Back/Next enable/disable correctly. Report before continuing.

- [ ] **Step 4: Commit**

```bash
git add rust/src/gui/ rust/src/main.rs
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: GUI wizard shell + Target page"
```

---

### Task 10: GUI ProcDump + Task pages

Pages follow the exact structural pattern established by `page_target.rs` (a `build(parent, state) -> Page` fn, `load`/`save` methods, events wired in `gui::run`'s handler). This task specifies controls + logic; the nwg builder boilerplate is mechanical repetition of Task 9's pattern.

**Files:**
- Create: `rust/src/gui/page_procdump.rs`, `rust/src/gui/page_task.rs`
- Modify: `rust/src/gui/mod.rs` (mod lines, page construction, save/load arms, event wiring)

**Interfaces:**
- Consumes: `procdump::{build_args, Preset}`, `task::{auto_task_name, sanitize_task_name, exists, query_status}`, `bitness`.
- Produces: `page_procdump::ProcDumpPage`, `page_task::TaskPage` with the standard `build/load/save` contract.

**ProcDump page — a `nwg::TabsContainer` with two tabs.**

Tab "Basic" controls (label x=10, control x=190 within tab):

| Control | Type | Binds to | Notes |
|---|---|---|---|
| `cmb_scenario` | ComboBox | `cfg.scenario` | Items: 5 preset names + "Custom". Selecting a preset calls `Preset::find(name).apply(&mut cfg)` then `refresh_all_controls()`; any manual edit of the option controls below sets combo to "Custom" and `cfg.scenario = ""` |
| `txt_effective` | TextInput readonly, width 540 | — | Live preview: `procdump::build_args(&cfg_from_controls())`; refresh on every control change (single `refresh_preview()` called from all handlers) |
| `lbl_bitness` | Label, width 540 | — | On page load: `bitness::select_binary(bitness::detect(&target), dir, true).summary` + warning if any |
| `txt_procdump_path` + `btn_browse_pd` | TextInput + Button "Browse..." | `cfg.proc_dump_path` | Browse = `nwg::FileDialog` (action Open, filter `Exe(*.exe)`) |
| `txt_dump_dir` + `btn_browse_dir` | TextInput + Button "Browse..." | `cfg.dump_directory` | `nwg::FileDialog` action OpenDirectory |
| `cmb_dump_type` | ComboBox | `cfg.dump_type` | Full / MiniPlus / Mini / ThreadDump |
| `chk_exception` "-e unhandled exception" | CheckBox | `cfg.dump_on_exception` | |
| `chk_hang` "-h hung window" | CheckBox | `cfg.hang_window_seconds` (checked=1, unchecked=0) | |
| `chk_terminate` "-t on terminate" | CheckBox | `cfg.dump_on_terminate` | |
| `txt_cpu` / `txt_cpu_low` / `txt_cpu_dur` / `txt_count` | TextInput (numeric, 60 wide) | `cpu_threshold` / `cpu_low_threshold` / `cpu_duration_seconds` / `max_dumps` | parse `.trim().parse::<i32>().unwrap_or(0)`; max_dumps floor 1 |
| `chk_cpu_per_unit` "-u per-CPU" | CheckBox | `cfg.cpu_per_unit` | |
| `txt_mem` "Commit MB (-m)" | TextInput | `cfg.memory_commit_mb` | |
| `chk_clone` "-r clone" / `chk_avoid` "-a avoid outage" / `chk_overwrite` "-o overwrite" / `chk_wait` "-w wait for launch" | CheckBox ×4 | `use_clone` / `avoid_outage` / `overwrite_existing` / `wait_for_process` | |
| `txt_restart_delay` / `txt_min_disk` | TextInput | `restart_delay_seconds` / `min_free_disk_mb` | |

Tab "Advanced" controls:

| Control | Binds to |
|---|---|
| `txt_perf_counter` (-p) | `cfg.performance_counter` |
| `txt_perf_threshold` (-pl) | `cfg.perf_counter_threshold` |
| `txt_filter_include` (-f) | `cfg.exception_filter_include` |
| `txt_filter_exclude` (-fx) | `cfg.exception_filter_exclude` |
| `chk_wer` (-wer) | `cfg.wer_integration` |
| `txt_avoid_terminate` (-at) | `cfg.avoid_terminate_timeout` |

Core logic that is NOT boilerplate — implement exactly:

```rust
impl ProcDumpPage {
    /// Any manual option edit flips the scenario to Custom.
    pub fn on_option_changed(&self, state: &super::WizardState) {
        if !self.suppress_custom.get() {                     // guard: preset application
            self.cmb_scenario.set_selection(Some(5));        // index 5 = "Custom"
            state.cfg.borrow_mut().scenario = String::new();
        }
        self.refresh_preview(state);
    }

    pub fn on_scenario_selected(&self, state: &super::WizardState) {
        let Some(i) = self.cmb_scenario.selection() else { return };
        if i < 5 {
            let preset = &crate::procdump::Preset::all()[i];
            self.suppress_custom.set(true);
            {
                let mut cfg = state.cfg.borrow_mut();
                self.save(&mut cfg);          // capture path fields the user already set
                preset.apply(&mut cfg);
                self.load(&cfg);              // push preset values back into controls
            }
            self.suppress_custom.set(false);
        }
        self.refresh_preview(state);
    }

    pub fn refresh_preview(&self, state: &super::WizardState) {
        let mut cfg = state.cfg.borrow().clone();
        self.save(&mut cfg);
        self.txt_effective.set_text(&crate::procdump::build_args(&cfg));
    }
}
```

(`suppress_custom: Cell<bool>` — without it, `load()` firing change events while applying a preset would immediately flip the combo back to Custom. This mirrors a real bug class from the C# app.)

**Task page controls:**

| Control | Type | Purpose |
|---|---|---|
| `txt_task_name` | TextInput width 400 | `cfg.task_name`; on save run `sanitize_task_name` |
| `btn_reset_auto` "Reset to Auto" | Button | `txt_task_name.set_text(&auto_task_name(&cfg.target_name))` |
| `lbl_exists` | Label bold width 540 | On `load`: `task::exists(name)` → "Task exists — it will be UPDATED." / "New task will be created." |
| `txt_existing` | TextInput readonly multiline 540×90 | Visible only when exists: `query_status` → State / LastRunTime / LastRunResult / NextRunTime lines |
| `txt_action_preview` | TextInput readonly multiline 540×70 | `EXE: {exe}\r\nArguments: --monitor --config "{cfg}"\r\nWork Dir: {dir}` from `paths::*` |
| `btn_copy_cmd` "Copy Command" | Button | clipboard via `nwg::Clipboard::set_data_text` |
| `lbl_props` | Label multiline | Static text: "Runs as SYSTEM · At startup · Restart 1 min ×999 · Ignore new instances · No time limit" |

- [ ] **Step 1: Implement both pages per spec above** (follow `page_target.rs` structure verbatim)
- [ ] **Step 2: Wire into `gui/mod.rs`** — page construction, save/load arms in the nav handler (`cur == 1` / `cur == 2`), events: scenario selection, every option control → `on_option_changed`, browse buttons, reset-auto, copy.
- [ ] **Step 3: Build + verify on VM**

Run: `scripts/vm-build.sh build && scripts/vm.sh 'Start-Process C:\pdm\target\release\ProcDumpMonitor.exe'`
**User checkpoint:** scenario dropdown applies presets (effective command matches the preset flags column from the README table), editing any option flips to Custom, browse dialogs work, task page shows exists/new correctly (create `PDM Rust Test` first via CLI to see the UPDATE path).

- [ ] **Step 4: Commit**

```bash
git add rust/src/gui/
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: GUI ProcDump + Task pages with scenario presets"
```

---

### Task 11: GUI Notify + Review + About pages

**Files:**
- Create: `rust/src/gui/page_notify.rs`, `rust/src/gui/page_review.rs`, `rust/src/gui/page_about.rs`
- Modify: `rust/src/gui/mod.rs`

**Interfaces:**
- Consumes: `notify::{send_test_email, validate_smtp_connectivity, split_addresses}`, `secrets`, `task`, `config`, `paths`.
- Produces: the three pages with the standard `build/load/save` contract.

**Notify page controls:**

| Control | Binds to | Notes |
|---|---|---|
| `chk_email` "Enable email notifications" | `cfg.email_enabled` | toggling enables/disables the email field group |
| `txt_smtp` / `txt_port` / `chk_ssl` | `smtp_server` / `smtp_port` / `use_ssl` | port parse u16, default 25 |
| `txt_from` / `txt_to` / `txt_cc` | `from_address` / `to_address` / `cc_address` | To/CC semicolon-delimited |
| `txt_user` | `smtp_username` | |
| `txt_password` (PasswordInput: `TextInput` with `.password(Some('•'))`) | → DPAPI | **Save logic:** if the field is non-empty, `cfg.encrypted_password_blob = secrets::protect(&text, secrets::SMTP_ENTROPY)` and clear the field; if empty, keep the existing blob. Load never populates it (placeholder text "(unchanged)" when a blob exists) — plaintext never round-trips |
| `btn_validate` "Validate SMTP" | — | `validate_smtp_connectivity(&server, port, 5000)` → result in `lbl_notify_status` |
| `btn_test_email` "Send Test Email" | — | `save()` into a cfg clone first, then `send_test_email(&clone)` → result in `lbl_notify_status` |
| `chk_webhook` "Enable webhook notifications" | `cfg.webhook_enabled` | |
| `txt_webhook` | → DPAPI | Same blob pattern: non-empty → `secrets::protect(&url, secrets::WEBHOOK_ENTROPY)` into `encrypted_webhook_url_blob`, clear `webhook_url` |
| Maintenance group: `txt_log_size` / `txt_log_files` / `txt_ret_days` / `txt_ret_gb` / `txt_stab_timeout` | `max_log_size_mb` / `max_log_files` / `dump_retention_days` / `dump_retention_max_gb` (f64) / `dump_stability_timeout_seconds` | |
| `lbl_notify_status` | — | width 720, shows validate/test results |

Email validation on save (port of `ValidateAddressList`, minimal): if `email_enabled` and (`from_address` empty or `split_addresses(&to_address)` empty or any To/CC entry lacks `'@'`), show a `nwg::modal_error_message` and stay on the page (return `false` from `save`; shell treats false = block navigation. Extend the page contract: `save(&self, cfg) -> bool`, all other pages return `true`).

**Review page:**

| Control | Purpose |
|---|---|
| `txt_summary` readonly multiline 740×180 | On `load`: target, scenario, effective args, dump dir, task name, email on/off + recipients, webhook on/off, retention lines — one `format!` block from `cfg` |
| Buttons row 1 (each 110×28): `btn_create` "Create Task" / `btn_run` "Run Task Now" / `btn_stop` "Stop Task" / `btn_remove` "Remove Task" | see actions below |
| Buttons row 2: `btn_save_only` "Save Config Only" / `btn_open_dumps` "Open Dump Folder" / `btn_view_logs` "View Logs" / `btn_copy_args` "Copy ProcDump Cmd" / `btn_taskschd` "Open Task Scheduler" | |
| `lbl_banner` | width 740, bold — "OK: ..." on success, "ERROR: ..." on failure (text prefix instead of color; nwg label recoloring is not worth the plumbing) |
| `lst_log` ListBox 740×160 | session log; every action appends a timestamped line |

Action implementations (in `page_review.rs`), **per spec: task ops shell out to this exe's own CLI verbs — one code path with headless usage**:

```rust
fn run_own_verb(verb: &str) -> (bool, String) {
    let out = std::process::Command::new(crate::paths::exe_path())
        .args([verb, "--config", &crate::paths::config_path().display().to_string()])
        .output();
    match out {
        Ok(o) => {
            let text = format!("{}{}",
                String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            (o.status.success(), text.trim().to_string())
        }
        Err(e) => (false, format!("cannot run {verb}: {e}")),
    }
}
```

- `btn_create`: save all pages into cfg → `cfg.save(&paths::config_path())` → `run_own_verb("install")` → banner + log. Button label shows "Update Task" when `task::exists()` (refresh on page load).
- `btn_run` / `btn_stop` / `btn_remove`: `run_own_verb("start" | "stop" | "uninstall")`.
- `btn_save_only`: `cfg.save(...)` only.
- `btn_open_dumps`: `Command::new("explorer.exe").arg(&cfg.dump_directory).spawn()`.
- `btn_view_logs`: `Command::new("notepad.exe").arg(paths::log_path()).spawn()`.
- `btn_copy_args`: clipboard ← `procdump::build_args(&cfg)`.
- `btn_taskschd`: `Command::new("mmc.exe").arg("taskschd.msc").spawn()`.

**About page:** static labels — app name (bold, larger `nwg::Font`), "A SWH L3 Production — packaged for C•CURE deployments.", `format!("Build {}  ·  v{}", env!("BUILD_DATE"), env!("CARGO_PKG_VERSION"))`, and the JCI globe: `nwg::ImageFrame` with `nwg::Bitmap::from_bin(include_bytes!("../../assets/jci_globe_256.png"))` (copy `Assets/jci_globe_256.png` → `rust/assets/`; requires nwg `image-decoder` feature — add `features = ["image-decoder"]` to the nwg dependency).

- [ ] **Step 1: Implement the three pages + contract change** (`save -> bool`, shell blocks Next/Back on false)
- [ ] **Step 2: Wire into `gui/mod.rs`** (construction, nav arms `cur == 3/4/5`, all button events)
- [ ] **Step 3: Build + full wizard walkthrough on VM**

Run: `scripts/vm-build.sh build && scripts/vm.sh 'Copy-Item C:\pdm\target\release\ProcDumpMonitor.exe C:\PDMTest\ -Force; Start-Process C:\PDMTest\ProcDumpMonitor.exe'`
**User checkpoint — full acceptance walk:** all 6 pages navigate; Notify blocks bad email; Review's Create Task → `schtasks /Query` shows SYSTEM task; Run Task Now → kill notepad → dump appears + session log updates; Remove Task cleans up; About shows logo + build date. This is the parity gate against the C# wizard.

- [ ] **Step 4: Commit**

```bash
git add rust/src/gui/ rust/assets/
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: GUI Notify/Review/About pages - full wizard complete"
```

---

### Task 12: release polish — size gate, docs, ship

**Files:**
- Modify: `README.md`, `rust/Cargo.toml` (only if size gate fails)
- Create: `dist/ProcDumpMonitor.exe` (final artifact)

- [ ] **Step 1: Final VM test suite + release build**

Run: `scripts/vm-build.sh test && scripts/vm-build.sh build`
Expected: all tests green (incl. DPAPI on VM), exe fetched.

- [ ] **Step 2: Size gate**

Run: `ls -la dist/ProcDumpMonitor.exe && du -h dist/ProcDumpMonitor.exe`
Expected: **≤ 6 MB** (spec's honest estimate 3–6 MB). If over: check `cargo bloat --release` on the VM (`cargo install cargo-bloat` there) — usual suspects are chrono TZ data and rustls; confirm `strip=true`, `panic="abort"`, `opt-level="z"` took effect. Do NOT chase below 3 MB — diminishing returns.

- [ ] **Step 3: Fresh-machine smoke test** (clean folder, like a real deployment)

```bash
scripts/vm.sh '
$d = "C:\FreshDeploy"
if (Test-Path $d) { Remove-Item $d -Recurse -Force }
mkdir $d | Out-Null
Copy-Item C:\pdm\target\release\ProcDumpMonitor.exe $d
Copy-Item C:\PDMTest\procdump64.exe $d
cd $d
.\ProcDumpMonitor.exe version
.\ProcDumpMonitor.exe status        # no config.json yet -> defaults, "Not installed"
"EXIT=$LASTEXITCODE"'
```

Expected: version prints, status JSON with `"Exists": false`, `EXIT=0`, and a `config.json`-free folder doesn't crash anything.

- [ ] **Step 4: Update README.md**

Rewrite the **Building**, **Requirements**, and **NuGet Dependencies** sections for the Rust build:
- Requirements: drop ".NET 8 SDK"; add "Rust (MSVC toolchain) — only if building from source".
- Building: `cd rust && cargo build --release` (on Windows) or `scripts/vm-build.sh` (from LRPC); output `rust/target/release/ProcDumpMonitor.exe`.
- Replace the NuGet table with the crate list from `rust/Cargo.toml`.
- CLI Reference: remove `--oneshot`, `--selftest`, `--support-diagnostics`, `--export-config`, `--no-elevate` rows (cut features; elevation now comes from the embedded manifest). Note verbs accept both `install` and `--install` forms.
- Config & Migration: replace with "Schema V3-compatible field names; **no automatic migration** — existing configs from the .NET version load field-for-field, but pre-V3 configs are not migrated." (Fields match, so a V3 config.json from the C# app WILL load — worth saying explicitly.)
- Add one line under Quick Start: single exe, no runtime install, ~5 MB.

- [ ] **Step 5: Final commit**

```bash
git add README.md dist/ rust/
git -c user.name="mattressburrn" -c user.email="chevyboxer@gmail.com" \
  commit -m "rust-rewrite: release build, size gate, README for Rust toolchain"
```

- [ ] **Step 6: Report to user**

Summarize: final exe size vs the old ~70–150 MB publish, test counts (Linux + VM), the parity walkthrough result, and the open question of whether/when to delete the C# sources (user's call — NOT part of this plan).

---

## Plan Self-Review (completed at write time)

- **Spec coverage:** GUI wizard 6 pages (T9–11), Rust+nwg (T1), schtasks XML path with proven landmines (T4), DPAPI LocalMachine both entropies (T7), bitness incl. Server 2016 GetProcAddress trap (T7), monitor loop with disk guard/retention/stability/dedup/health heartbeat (T5, T8), email TLS modes + webhook MessageCard (T6), CLI verbs + exit codes (T8), cut-list respected (no oneshot/selftest/diag/migration/themes), Linux-vs-VM test split throughout, egui fallback gate (T1 Step 6). Config schema = C# V3 minus `RemoveTaskAfterSuccessfulDump` (cut with --oneshot).
- **Deviations from spec, both deliberate:** (1) `chrono` + `base64` crates added beyond the spec's dependency list — local-time log/email timestamps and DPAPI blob encoding; hand-rolling either is the flimsier choice. (2) Webhook URL stays DPAPI-encrypted like the C# app (spec's schema line said "webhook URL"; keeping encryption preserves current security posture at near-zero cost — same module, second entropy constant).
- **Type consistency check:** `Config` field names/types match between T2 definition and all later consumers; `TaskStatus` serde names match the C# `CliStatusOutput`; `Preset::apply` signature consistent T3→T10; `save() -> bool` contract change is called out where introduced (T11) and noted as an extension of T9's contract.
- **Placeholder scan:** none remaining; GUI pages 2–6 specify controls by table + non-boilerplate logic in full, with T9's Target page as the complete structural exemplar (deliberate — repeating ~400 lines of positional builder code per page adds no information an implementer needs).





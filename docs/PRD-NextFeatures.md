# ProcDump Monitor — Mini PRD & Technical Plan

**Date:** 2026-02-28  
**Scope:** Next feature increment for ProcDump Monitor v2  
**Constraint:** Single-file self-contained `win-x64`, offline-capable, no breaking config changes

---

## 1. Feature Shortlist

| # | Feature | Impact | Effort | Rationale |
|---|---------|--------|--------|-----------|
| 1 | **Dump-file stability check before notification** | High | S | Current code emails immediately after ProcDump exits. If ProcDump crashes mid-write you email about a partial/corrupt dump. Poll `FileStream.Open(ReadWrite, None)` + stable-size check to confirm the file is complete. |
| 2 | **Config schema versioning & auto-migration** | High | S | Adding any field today silently defaults. A `"version": 2` field plus a `ConfigMigrator` class lets you evolve the schema safely and warn on downgrade. |
| 3 | **Heartbeat / watchdog health file** | High | S | Write a `health.json` (PID, last-cycle UTC, dump count) every loop. External monitoring (SCOM, Zabbix, or a simple `Test-Path` script) can detect a stalled monitor without parsing logs. |
| 4 | **Disk-space guard** | High | S | Full-memory dumps are multi-GB. Before launching ProcDump, check free space on the dump volume. Skip the cycle and log/email a warning if below a configurable threshold (e.g. 5 GB). Prevents filling a disk and crashing the server. |
| 5 | **Multiple email recipients (To + CC)** | Medium | S | `ToAddress` is currently a single string. Accept semicolon-delimited addresses. Trivial change, high field value — ops teams have more than one on-call. |
| 6 | **CLI silent-install / update / status** | High | M | Enterprise deployment needs `ProcDumpMonitor.exe --install --config path`, `--status`, `--uninstall` without a GUI. Maps directly to existing `TaskSchedulerService` methods. |
| 7 | **Configurable ProcDump trigger flags** | High | M | Expose `-c` (CPU threshold), `-m` (memory commit), `-cl` (CPU low), `-p` (performance counter) in config and UI. Each maps to a ProcDump flag. Requires safety warnings (see §4). |
| 8 | **Log rotation** | Medium | S | `procdump.log` grows unbounded. Rotate at a configurable size (default 10 MB), keep N old files, delete oldest. |
| 9 | **Dump-file retention / auto-cleanup** | Medium | M | Delete or compress dumps older than N days or when total folder size exceeds a cap. Prevents runaway disk usage on boxes that crash frequently. |
| 10 | **Webhook / Teams / Slack notification channel** | Medium | M | Many SOCs use Teams incoming webhooks. Add an `INotifier` abstraction and a `WebhookNotifier` alongside `EmailNotifier`. Offline: gracefully skip and log. |
| 11 | **Export / import config** | Medium | S | "Export Config…" / "Import Config…" buttons. Useful when deploying the same settings to 50 servers. Redacts `EncryptedPasswordBlob` on export. |
| 12 | **Per-cycle summary with dump hash** | Low | S | Log SHA-256 of the completed dump. Proves integrity when a dump is moved off-box for analysis. |
| 13 | **Dark/Light theme toggle** | Low | S | Current theme is hard-coded dark. Add a toggle; persist preference in config. |
| 14 | **Tray icon for GUI mode** | Low | M | Minimize to system tray with balloon notification on dump. Not useful for headless mode but helps interactive troubleshooting. |
| 15 | **Multi-target monitoring** | Low | L | Monitor more than one process from a single config (array of targets, one ProcDump child per target). Significant refactor to the monitor loop and config model. Defer to a later release. |

---

## 2. Top 5 Features — Acceptance Criteria

### F1 — Dump-File Stability Check

| AC# | Criterion |
|-----|-----------|
| 1.1 | After ProcDump exits, the monitor polls the newest `.dmp` file every 2 s, up to 30 s, until `new FileStream(path, Open, Read, None)` succeeds and the file size is unchanged across two consecutive polls. |
| 1.2 | If the file is still locked after the timeout, the event is logged as `"Dump file still locked — skipping notification"` and the email is **not** sent. |
| 1.3 | If no `.dmp` file exists at all, existing behaviour is preserved (`"No new dump file detected"`). |
| 1.4 | Unit test: mock `FileStream` open that throws `IOException` twice then succeeds → notification proceeds. |
| 1.5 | Unit test: mock that always throws → notification is suppressed, log message written. |

### F2 — Config Schema Versioning & Migration

| AC# | Criterion |
|-----|-----------|
| 2.1 | `config.json` gains a root `"configVersion": 2` property (integer). Files without the field are treated as version 1. |
| 2.2 | `ConfigMigrator.Migrate(json)` returns a `Config` at the latest version, applying transforms V1→V2 (add defaults for new fields). |
| 2.3 | On save the current version number is always written. |
| 2.4 | If a file has a **higher** version than the running binary, a warning dialog is shown: `"Config was created by a newer version. Some settings may be ignored."` |
| 2.5 | Existing `config.json` files from the current release load without error and produce identical behaviour. |

### F3 — Disk-Space Guard

| AC# | Criterion |
|-----|-----------|
| 3.1 | New config field `MinFreeDiskMB` (default `5120`, i.e. 5 GB). |
| 3.2 | Before each ProcDump launch, the monitor calls `DriveInfo` on the dump volume. If free space < threshold, the cycle is skipped. |
| 3.3 | A log line is written: `"Skipping cycle — only X MB free on Y: (threshold: Z MB)"`. |
| 3.4 | If email is enabled, a single "low disk" warning email is sent per hour (deduped by a cooldown timer), not per cycle. |
| 3.5 | GUI displays an editable *Min Free Disk (MB)* field in the ProcDump Settings group. |

### F4 — CLI Silent Install / Update / Status

| AC# | Criterion |
|-----|-----------|
| 4.1 | `--install --config <path>` (or `--update`) creates/updates the Scheduled Task using the supplied config, prints result to stdout, and exits with code 0 on success, 1 on failure. |
| 4.2 | `--status` prints a JSON object with task existence, state, last run, last result, next run to stdout and exits 0. |
| 4.3 | `--uninstall` removes the Scheduled Task, prints confirmation, exits 0 (or 1 if task not found). |
| 4.4 | All CLI paths work without loading WinForms assemblies (no `ApplicationConfiguration.Initialize()`). |
| 4.5 | Exit codes and JSON output are documented in `--help`. |
| 4.6 | Integration test: run `--install`, then `--status`, then `--uninstall` in sequence on a clean machine; verify each exit code and JSON shape. |

### F5 — Configurable ProcDump Trigger Flags

| AC# | Criterion |
|-----|-----------|
| 5.1 | New config fields: `CpuThreshold` (int, 0 = disabled), `MemoryCommitMB` (int, 0 = disabled), `CpuLowThreshold` (int, 0 = disabled), `HangWindowSeconds` (int, 0 = disabled). |
| 5.2 | `BuildProcDumpArgs()` emits `-c <N>` when `CpuThreshold > 0`, `-cl <N>` when `CpuLowThreshold > 0`, `-m <N>` when `MemoryCommitMB > 0`, `-h` when `HangWindowSeconds > 0`. |
| 5.3 | GUI: new section *"Advanced Triggers"* with numeric inputs and tooltips explaining each flag. A ⚠ warning label reads: `"Aggressive thresholds (e.g. CPU ≥ 5%) may produce dumps every few seconds. Use with caution."` |
| 5.4 | When any advanced trigger is enabled, `-e` and `-t` remain active unless the user explicitly unchecks them (additive, not exclusive). |
| 5.5 | Unit test: `BuildProcDumpArgs()` with `CpuThreshold=80, MemoryCommitMB=4096` → args contain `-c 80 -m 4096`. |

---

## 3. Updated Configuration Model

### Schema Changes (V1 → V2)

```jsonc
{
  // ── NEW: schema version ──
  "configVersion": 2,

  // ── Existing (unchanged) ──
  "TargetName": "SoftwareHouse.CrossFire.Server",
  "ProcDumpPath": "C:\\Tools\\ProcDumpMonitor\\procdump64.exe",
  "DumpDirectory": "C:\\Dumps\\CrossFire",
  "DumpType": "Full",
  "DumpOnException": true,
  "DumpOnTerminate": true,
  "UseClone": true,
  "MaxDumps": 1,
  "RestartDelaySeconds": 5,
  "TaskName": "ProcDump Monitor - CrossFire",

  // ── NEW: advanced ProcDump triggers (F5) ──
  "CpuThreshold": 0,          // 0 = disabled; 1-100 maps to -c <N>
  "CpuLowThreshold": 0,       // maps to -cl <N>
  "MemoryCommitMB": 0,        // maps to -m <N>
  "HangWindowSeconds": 0,     // maps to -h

  // ── NEW: disk-space guard (F3) ──
  "MinFreeDiskMB": 5120,

  // ── NEW: log rotation (F8 — bonus) ──
  "MaxLogSizeMB": 10,
  "MaxLogFiles": 5,

  // ── Email (existing + enhanced) ──
  "EmailEnabled": true,
  "SmtpServer": "smtp.example.com",
  "SmtpPort": 25,
  "UseSsl": false,
  "FromAddress": "alerts@example.com",
  "ToAddress": "oncall@example.com; oncall2@example.com",   // ← now semicolon-delimited
  "SmtpUsername": "",
  "EncryptedPasswordBlob": "<REDACTED>"
}
```

### Migration Rules

| From → To | Transform |
|-----------|-----------|
| (none) → 2 | Add `configVersion: 2`; all new fields get defaults shown above. `ToAddress` stays as-is (single address is valid semicolon-delimited). |

### Source-Gen Context Update

```csharp
[JsonSerializable(typeof(Config))]
[JsonSerializable(typeof(HealthStatus))]   // new type for health.json
internal partial class ConfigJsonContext : JsonSerializerContext { }
```

---

## 4. Implementation Plan

### 4.1 New / Modified Files

| File | Change Type | Description |
|------|-------------|-------------|
| **Config.cs** | Modify | Add new properties (`ConfigVersion`, `CpuThreshold`, `CpuLowThreshold`, `MemoryCommitMB`, `HangWindowSeconds`, `MinFreeDiskMB`, `MaxLogSizeMB`, `MaxLogFiles`). Update `BuildProcDumpArgs()` to emit new flags. |
| **ConfigMigrator.cs** | **New** | Static class: `Migrate(string json) → Config`. Reads `configVersion`, applies transforms, returns hydrated `Config`. |
| **DumpFileWaiter.cs** | **New** | Static helper: `WaitForStableFile(string path, TimeSpan timeout) → bool`. Polls for exclusive-read access and stable file size. |
| **DiskSpaceGuard.cs** | **New** | Static helper: `CheckFreeSpace(string dumpDir, long minMB) → (bool ok, long actualMB)`. |
| **HealthStatus.cs** | **New** | POCO: `Pid`, `LastCycleUtc`, `DumpsCaptured`, `Version`. Written to `health.json` via source-gen JSON. |
| **ProcDumpMonitor.cs** | Modify | Integrate `DiskSpaceGuard` check before launch. Call `DumpFileWaiter` before notification. Write `health.json` each cycle. |
| **Logger.cs** | Modify | Add `RotateIfNeeded()` called at start of `Log()`. Checks file size, renames to `.1`, shifts older files, deletes oldest beyond `MaxLogFiles`. |
| **EmailNotifier.cs** | Modify | Parse semicolon-delimited `ToAddress` into multiple `MailAddress`. Add `SendLowDiskWarning()`. |
| **MainForm.cs** | Modify | Add UI controls for new config fields (Advanced Triggers group, Min Free Disk field). Add Export/Import buttons. |
| **Program.cs** | Modify | Add `--install`, `--update`, `--uninstall`, `--status`, `--help` CLI branches that call `TaskSchedulerService` and exit without WinForms init. |
| **TaskSchedulerService.cs** | No change | Already has `InstallOrUpdate`, `RemoveTask`, `GetDetailedStatus` — CLI just calls them. |

### 4.2 Module Interaction (Monitor Loop)

```
ProcDumpMonitorLoop.Run(cfg)
│
├─ each cycle:
│   ├─ HealthStatus.Write(health.json)
│   ├─ DiskSpaceGuard.CheckFreeSpace() ──► skip + warn if low
│   ├─ Launch ProcDump (existing)
│   ├─ Wait for exit (existing)
│   ├─ DumpFileWaiter.WaitForStableFile(newestDmp)
│   │   └─ polls for exclusive access + stable size (2 s interval, 30 s max)
│   ├─ Log SHA-256 hash of dump
│   └─ EmailNotifier.SendDumpNotification() ──► semicolon-split To
│
└─ Logger.Log() ──► RotateIfNeeded()
```

### 4.3 CLI Routing (Program.cs)

```
Main(args)
├─ --help                    → print usage, exit 0
├─ --monitor --config <path> → ProcDumpMonitorLoop.Run() (existing)
├─ --install --config <path> → Config.Load() → .Save() → TaskSchedulerService.InstallOrUpdate(), exit 0/1
├─ --update  --config <path> → (alias for --install)
├─ --uninstall               → TaskSchedulerService.RemoveTask(), exit 0/1
├─ --status                  → TaskSchedulerService.GetDetailedStatus() → JSON to stdout, exit 0
└─ (no args)                 → WinForms GUI (existing)
```

### 4.4 ProcDump Flag → Config Field Mapping

| Config Field | ProcDump Flag | Meaning | Safety Warning |
|--------------|---------------|---------|----------------|
| `DumpOnException` | `-e` | Dump on unhandled exception | Generally safe; one dump per crash. |
| `DumpOnTerminate` | `-t` | Dump on process termination | Safe for investigating why a process exits. |
| `UseClone` | `-r` | Clone process before dumping | Recommended for production — avoids suspending the target. |
| `CpuThreshold` | `-c <N>` | Dump when CPU% exceeds N for 10 s | ⚠ Low thresholds cause rapid repeated dumps. Default to 0 (off). Warn if < 30. |
| `CpuLowThreshold` | `-cl <N>` | Dump when CPU% drops below N | Niche use (detecting freezes). Warn that this triggers when the process is *idle*. |
| `MemoryCommitMB` | `-m <N>` | Dump when commit exceeds N MB | ⚠ Must be set well above normal working set. Too low = dump on startup. |
| `HangWindowSeconds` | `-h` | Dump when a window stops responding for N s | Only works for GUI processes with a message loop. Not useful for services. |

---

## 5. Risk Analysis

### 5.1 Security

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `config.json` read by non-admin → leak DPAPI blob | Medium | Medium | Blob is only decryptable on the same machine. Document: restrict folder ACL to Administrators + SYSTEM. |
| CLI `--install` with world-readable config path | Low | Medium | CLI validates that config file has restrictive ACL (warn if `Everyone` has read). |
| SMTP password visible in memory dump of the monitor process | Low | Low | Inherent to any process holding credentials. Use `SecureString` if .NET deprecation allows, or document risk. |
| Webhook URL leaked in config export | Medium | Medium | `Export Config` redacts `EncryptedPasswordBlob` and webhook URLs. |

### 5.2 Reliability

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Monitor loop crashes, no dumps captured | Medium | High | Scheduled Task has `RestartCount=999, RestartInterval=1 min`. Health file goes stale → external alert. |
| Dump disk fills up, OS crashes | High | Critical | `DiskSpaceGuard` skips cycle and emails warning. Dump-retention auto-cleanup deletes old dumps. |
| ProcDump hangs (e.g., target never starts) | Low | Medium | Already mitigated: `-w` (wait) is non-blocking to the OS; monitor loop `WaitForExit(1000)` + `_stopping` flag can kill it. Consider adding a per-cycle timeout (configurable, default 24 h). |
| Log file grows unbounded under heavy crash rate | Medium | Low | Log rotation caps at `MaxLogSizeMB × MaxLogFiles`. |
| Config migration bug loses settings | Low | High | Migrate in-memory, write backup `config.json.bak` before overwriting. Unit test every V(n)→V(n+1) path. |

### 5.3 UX

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| User sets CPU threshold = 5%, gets hundreds of dumps | Medium | Medium | GUI warning label. Validate: if < 30, show confirmation dialog. |
| CLI `--install` silently overwrites running task | Medium | Low | Print warning: `"Task 'X' is currently Running — it will be replaced. Stop the task first to avoid orphaned ProcDump processes."` |
| Advanced trigger fields intimidate basic users | Low | Low | Collapse into an expandable "Advanced Triggers" panel, hidden by default. |

### 5.4 Operational

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Different DPAPI machine key after OS re-image | Medium | Medium | Password decryption fails silently (returns `""`). Log a clear error: `"SMTP password could not be decrypted — re-enter in GUI or via CLI."` |
| Single-file publish grows with new dependencies | Low | Low | No new NuGet packages required. `System.Net.Http` (for webhooks) is already in the runtime. |
| Trimmer strips new code paths | Medium | Medium | All new types are serialization-source-gen'd. Run trimmed publish + smoke test as part of CI. |

---

## 6. Test Plan

### 6.1 Unit Tests (xUnit, no OS dependencies)

| ID | Test | Target |
|----|------|--------|
| U1 | `BuildProcDumpArgs` with all trigger combos → correct flag string | `Config.BuildProcDumpArgs()` |
| U2 | `ConfigMigrator` V1 JSON (no `configVersion`) → Config with version 2 and defaults | `ConfigMigrator.Migrate()` |
| U3 | `ConfigMigrator` future version → returns config + sets `DowngradeWarning` flag | `ConfigMigrator` |
| U4 | `DiskSpaceGuard` with mocked `DriveInfo` below threshold → returns `(false, actual)` | `DiskSpaceGuard` |
| U5 | `DumpFileWaiter` with file that becomes accessible on 2nd poll → returns `true` | `DumpFileWaiter` |
| U6 | `DumpFileWaiter` with perpetually locked file → returns `false` after timeout | `DumpFileWaiter` |
| U7 | `Logger.RotateIfNeeded` rotates when size exceeds limit | `Logger` |
| U8 | Semicolon-delimited `ToAddress` parses to N `MailAddress` objects | `EmailNotifier` |
| U9 | `TaskNameHelper.Sanitize` with edge cases (empty, all invalid chars, 300-char string) | `TaskNameHelper` |
| U10 | `HealthStatus` round-trip serialize/deserialize via source-gen JSON | `HealthStatus` |

### 6.2 Integration Tests (require Windows, admin, ProcDump binary)

| ID | Test | Steps |
|----|------|-------|
| I1 | **CLI install round-trip** | `--install --config test.json` → `--status` (JSON, expect `Exists=true`) → `--uninstall` → `--status` (`Exists=false`). |
| I2 | **Monitor loop with test crasher** | Launch `DiagRunner.exe` (test app that crashes on demand), run monitor for one cycle, verify `.dmp` created and email sent (use Papercut SMTP). |
| I3 | **Disk-space guard** | Set `MinFreeDiskMB` to `999999999`, run one cycle, verify log says skipped and no dump attempted. |
| I4 | **Dump stability check** | Create a `.dmp` file that is write-locked by another process, start a monitor cycle, verify notification is suppressed until lock is released. |
| I5 | **Log rotation** | Set `MaxLogSizeMB=0.001`, write many log lines, verify rotation to `.1`, `.2`, etc. and deletion beyond `MaxLogFiles`. |
| I6 | **Config migration** | Place a V1 `config.json` (no `configVersion`), launch GUI, verify all new fields populated with defaults, save, verify `configVersion: 2`. |

### 6.3 Manual Validation Matrix

| Scenario | Win 10 22H2 | Win 11 24H2 | Server 2016 | Server 2022 |
|----------|:-----------:|:-----------:|:-----------:|:-----------:|
| GUI launches, dark theme renders | ☐ | ☐ | ☐ | ☐ |
| Install task, reboot, dump captured | ☐ | ☐ | ☐ | ☐ |
| CLI `--install` + `--status` | ☐ | ☐ | ☐ | ☐ |
| Email with semicolon-delimited recipients | ☐ | ☐ | ☐ | ☐ |
| Low-disk warning email sent (once/hr) | ☐ | ☐ | ☐ | ☐ |
| `health.json` updated each cycle | ☐ | ☐ | ☐ | ☐ |
| Published single-file EXE < 50 MB | ☐ | ☐ | ☐ | ☐ |
| Trimmed build: all features functional | ☐ | ☐ | ☐ | ☐ |

---

## 7. CLI Switches for Enterprise Deployment

```
ProcDumpMonitor.exe [options]

GUI (default):
  (no arguments)              Launch the WinForms configuration UI.

Monitor (headless):
  --monitor --config <path>   Run the continuous ProcDump monitor loop.

Task Management (silent, no GUI):
  --install --config <path>   Create or update the Scheduled Task from config.
  --update  --config <path>   Alias for --install.
  --uninstall                 Remove the Scheduled Task (reads TaskName from default config).
  --uninstall --config <path> Remove the Scheduled Task using TaskName from <path>.

Status:
  --status                    Print task status as JSON to stdout; exit 0.
  --status --config <path>    Use TaskName from <path>.

Miscellaneous:
  --help                      Print this help text and exit.
  --version                   Print assembly version and exit.
  --export-config <out>       Export config to <out> with secrets redacted.
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Operation failed (details on stderr) |
| 2 | Bad arguments / missing config file |

### Enterprise Deployment Example (PowerShell)

```powershell
# Deploy to remote server via PSRemoting
$dest = "\\SERVER01\C$\Tools\ProcDumpMonitor"
Copy-Item .\ProcDumpMonitor.exe, .\procdump64.exe, .\config.json -Destination $dest

Invoke-Command -ComputerName SERVER01 -ScriptBlock {
    & "C:\Tools\ProcDumpMonitor\ProcDumpMonitor.exe" --install --config "C:\Tools\ProcDumpMonitor\config.json"
    & "C:\Tools\ProcDumpMonitor\ProcDumpMonitor.exe" --status
}
```

### SCCM / Intune One-Liner

```cmd
ProcDumpMonitor.exe --install --config "%~dp0config.json"
```

---

## Appendix A: Implementation Priority & Sprint Plan

| Sprint | Features | Estimate |
|--------|----------|----------|
| **1** (foundation) | F2 Config versioning, F1 Dump stability check, F8 Log rotation | 3–4 days |
| **2** (reliability) | F3 Disk-space guard, F4 CLI switches, Heartbeat health file | 4–5 days |
| **3** (triggers & UX) | F5 Advanced ProcDump triggers, F5 multiple recipients, Export/Import | 3–4 days |
| **4** (polish & test) | Integration tests, manual validation matrix, README update | 2–3 days |

Total estimate: **~2–3 weeks** for one developer.

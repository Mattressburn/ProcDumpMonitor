# ProcDump Monitor — Engineering Codebase Report

> **Audience:** Software Engineering Managers, Security Reviewers, Packaging Teams
> **Date:** Auto-generated — see `BuildInfo.BuildDate` for the compiled build stamp.

---

## 1. High-Level Purpose

ProcDump Monitor is a Windows desktop utility that **configures and manages Microsoft's Sysinternals ProcDump as an unattended, scheduled monitoring task**. It is designed for support and engineering teams who need to capture crash dumps from production Windows services without manual intervention.

### Operational Model

```
┌─────────────────────────────────────────────────────────────────┐
│  GUI Wizard (MainForm)                                          │
│  User selects: Target → ProcDump options → Task → Notifications │
│  Output: config.json + Windows Scheduled Task                   │
└──────────────────┬──────────────────────────────────────────────┘
                   │ Registers task (SYSTEM, BootTrigger)
                   ▼
┌──────────────────────────────────────────────────────────────────┐
│  Scheduled Task  (runs ProcDumpMonitor.exe --monitor)            │
│  → ProcDumpMonitorLoop: launch ProcDump → wait → detect dumps   │
│  → DumpStabilityChecker: poll until .dmp file size stabilises    │
│  → DiskSpaceGuard: skip cycle if free disk < threshold          │
│  → RetentionPolicy: age- and size-based cleanup                 │
│  → NotificationQueue → EmailNotifier / WebhookNotifier          │
│  → HealthWriter: write health.json each cycle                   │
│  → Restart ProcDump after configurable delay                    │
└──────────────────────────────────────────────────────────────────┘
```

Key states:

| Mode | Entry Point | Description |
|------|-------------|-------------|
| **GUI** | `Program.Main()` with no args | Opens the wizard form |
| **Monitor** | `--monitor --config <path>` | Long-running loop that invokes ProcDump |
| **OneShot** | `--oneshot [--simulate-dump]` | Single cycle for CI / validation |
| **CLI ops** | `--install`, `--uninstall`, `--status`, `--start`, `--stop` | Manage the scheduled task |
| **Self-test** | `--selftest` | Quick smoke test (exit code 0 = pass) |
| **Diagnostics** | `--support-diagnostics` | Packages logs + config + system info into a ZIP |

---

## 2. Repository Structure

```
ProcDumpMonitor/
├── Program.cs                  Entry point: CLI dispatcher and GUI launcher
├── MainForm.cs                 WinForms wizard host (header, step indicator, navigation)
├── WizardPage.cs               Abstract base for all wizard pages
├── Pages/
│   ├── TargetPage.cs           Step 1 — process/service selection
│   ├── ProcDumpPage.cs         Step 2 — scenario presets + ProcDump option configuration
│   ├── TaskPage.cs             Step 3 — scheduled task name + existing-task detection
│   ├── NotificationsPage.cs    Step 4 — email (SMTP) and webhook configuration
│   ├── ReviewPage.cs           Step 5 — summary, create/update task, one-shot test
│   └── AboutPage.cs            Step 6 — branding and version information
├── Config.cs                   POCO model, DPAPI helpers, BuildProcDumpArgs(), Load/Save
├── ConfigMigrator.cs           V1→V2→V3 schema migration, downgrade warning
├── ConfigExportImport.cs       Export (secrets redacted) and import with migration
├── ProcDumpPreset.cs           Named scenario presets (Crash, Hang, CPU, Memory, Low Impact)
├── ProcDumpMonitor.cs          --monitor loop: launch ProcDump, detect dumps, notify
├── OneShotRunner.cs            --oneshot: single-cycle runner with interface abstractions
├── OneShotSelfTest.cs          --selftest: automated smoke check
├── TaskSchedulerService.cs     Create/update/remove Windows Scheduled Task (SYSTEM + BootTrigger)
├── TaskNameHelper.cs           Sanitize task names for Task Scheduler constraints
├── EmailNotifier.cs            MailKit-based SMTP sending with multi-recipient support
├── WebhookNotifier.cs          HTTP POST webhook notifications
├── NotificationQueue.cs        Deduplication and rate-limiting for notifications
├── HealthWriter.cs             Atomic health.json heartbeat file
├── DumpStabilityChecker.cs     Poll .dmp size until stable (avoid reading partial dump)
├── DiskSpaceGuard.cs           Free-space check before ProcDump launch
├── RetentionPolicy.cs          Age- and size-based dump file cleanup
├── Logger.cs                   Thread-safe file logger with size-based rotation
├── AppPaths.cs                 Canonical paths (config, health, logs) for single-file publish
├── BuildInfo.cs                Assembly metadata (build date) via MSBuild property
├── ThemeManager.cs             Dark theme application for all WinForms controls
├── ElevationHelper.cs          UAC detection and silent re-launch
├── ProcDumpBitnessResolver.cs  Detect target process bitness → select procdump.exe vs procdump64.exe
├── ProcDumpOptionsValidator.cs Validate ProcDump flag combinations before save
├── ProcDumpOptionCatalog.cs    Descriptive metadata for all ProcDump flags
├── StepIndicator.cs            Custom control: horizontal step progress bar
├── ThemedCheckBox.cs           Owner-drawn dark-themed checkbox
├── SupportDiagnosticsService.cs Collect logs, config, system info into a support ZIP bundle
├── Assets/
│   ├── jci_globe.ico           Application icon (window + taskbar)
│   ├── jci_globe_256.png       About page logo (JCI globe)
│   ├── logo192.png             Legacy logo (unused — retained for reference)
│   └── logo512.png             Legacy logo (unused — retained for reference)
├── tests/
│   └── ProcDumpMonitor.Tests/
│       ├── PresetTests.cs      Scenario preset flag generation
│       ├── ConfigTests.cs      Migration + export/import round-trip
│       ├── ValidationTests.cs  TaskNameHelper sanitization + flag generation
│       ├── OneShotRunnerTests.cs  OneShot simulation with mock interfaces
│       ├── BitnessResolverTests.cs  Bitness detection logic
│       └── RetentionPolicyTests.cs  Age/size retention cleanup
└── docs/
    └── EngineeringCodebaseReport.md  (this file)
```

---

## 3. Configuration Model

### Persistence

- **Location:** `config.json` stored next to the executable (`AppPaths.ConfigPath`).
- **Format:** JSON, serialized via `System.Text.Json` source generators (`ConfigJsonContext`) for trim-safe AOT compatibility.
- **Schema version:** `Config.CurrentVersion` (currently `3`). Stamped on every save.

### Migration

`ConfigMigrator.Migrate(json)` handles:

| From | To | Actions |
|------|----|---------|
| V0 (unversioned) | V3 | Stamp version; all new fields get property-initializer defaults |
| V1/V2 | V3 | Set `WaitForProcess = true` (matches V2 implicit behavior); stamp version |
| V(future) | V3 | Load with warning (`DowngradeWarning`); unknown fields ignored by STJ |

A `.bak` copy is created before overwriting an older-format config.

### Redaction

`ConfigExportImport.Export()` replaces `EncryptedPasswordBlob` and `WebhookUrl` with `<REDACTED>` before writing. Import clears the markers and forces notifications off.

### Scenario Default

New and migrated configs default `Scenario` to `"Crash capture"`. The UI never auto-selects "Custom".

---

## 4. Scheduled Task Behavior

### Creation

`TaskSchedulerService.InstallOrUpdate(cfg)` registers a task via the `TaskScheduler` COM library wrapper:

| Property | Value |
|----------|-------|
| **Principal** | `SYSTEM` service account, highest run level |
| **Trigger** | `BootTrigger` (runs at system startup) |
| **Action** | `ProcDumpMonitor.exe --monitor --config "<configPath>"` |
| **Restart** | 1-minute interval, 999 retries |
| **Instances** | `IgnoreNew` (single instance only) |
| **Time limit** | None (`TimeSpan.Zero`) |
| **Battery** | Runs on battery, does not stop on battery |

### Task Name

Auto-generated as `ProcDump Monitor - <ProcessName>`, sanitized by `TaskNameHelper.Sanitize()`:
- Invalid characters (`\ / : * ? " < > |`) replaced with dashes
- Repeated whitespace/dash runs collapsed
- Leading/trailing whitespace trimmed
- Capped at 200 characters

The user can override the name; a "Reset to Auto" button restores the generated value.

### Run-As Identity

Tasks run as **SYSTEM**, which ensures:
- No interactive logon required
- Survives user logoff
- Full access to local processes

---

## 5. Security Notes

### DPAPI Credential Storage

- SMTP passwords are encrypted with `ProtectedData.Protect()` using `DataProtectionScope.LocalMachine`.
- Entropy: `"ProcDumpMonitor-SMTP-v1"` (hardcoded).
- Encrypted blob is stored as Base64 in `config.json` (`EncryptedPasswordBlob`).
- **Risk:** Any administrator on the same machine can decrypt the blob (LocalMachine scope). This is acceptable for a SYSTEM-level monitoring tool, but the documentation should note it.

### Secrets Redaction

- Export always redacts `EncryptedPasswordBlob` and `WebhookUrl`.
- Support diagnostics bundles include config but the password blob is present in the bundle copy — consider redacting in future.

### Elevation

- GUI mode silently re-launches elevated via UAC (`ElevationHelper.RelaunchElevated()`).
- CLI commands that touch Task Scheduler require an elevated prompt; the tool prints an error and exits if not elevated.

---

## 6. Test Strategy

Tests are in `tests/ProcDumpMonitor.Tests/` (xUnit, .NET 8):

| Test Class | Coverage Area |
|------------|---------------|
| `PresetTests` | All 5 scenario presets produce correct ProcDump flags; reset clears previous triggers; paths preserved |
| `ConfigTests` | V1→V3 migration; downgrade warning; export/import round-trip with secret redaction |
| `ValidationTests` | `TaskNameHelper.Sanitize` edge cases; `BuildProcDumpArgs` flag generation; email address validation |
| `OneShotRunnerTests` | OneShot simulation with mock `ITaskSchedulerOps`, `IProcDumpRunner`, `IEmailSender` |
| `BitnessResolverTests` | 32-bit / 64-bit process detection and binary selection |
| `RetentionPolicyTests` | Age-based and size-based dump cleanup |

**Notable gaps:**
- No UI / integration tests (WinForms makes this difficult without a UI test framework).
- No tests for `ConfigMigrator` setting `Scenario = "Crash capture"` on empty configs (recommended addition).
- Notification pipeline (email, webhook) tested only via mocks — no live SMTP tests.

---

## 7. Known Risks and Recommended Hardening

| Area | Risk | Recommendation |
|------|------|----------------|
| **Code signing** | Unsigned EXE triggers SmartScreen and AV warnings | Sign with a code-signing certificate before shipping |
| **DPAPI scope** | LocalMachine scope means any admin can decrypt SMTP password | Document the threat model; consider CurrentUser scope if task runs under a service account with a profile |
| **Config on disk** | `config.json` is plaintext next to the EXE | File ACLs should restrict to Administrators + SYSTEM |
| **Error swallowing** | Several `catch { }` blocks silently swallow exceptions | Add logging to bare catch blocks; consider structured error reporting |
| **Single-file extraction** | `IncludeNativeLibrariesForSelfExtract` extracts to a temp folder | Monitor for AV false positives on temp extraction |
| **Update mechanism** | No auto-update or version check | Consider a version manifest endpoint or MSI wrapper |
| **Log growth** | Logger rotates, but `Logs/` directory could grow if rotation parameters are misconfigured | Add a maximum total log size guard |
| **Task Scheduler COM** | `TaskScheduler` NuGet package uses COM interop | Ensure `BuiltInComInteropSupport = true` and trimmer roots are maintained |
| **Webhook security** | Webhook URL stored in plaintext | Consider DPAPI encryption for the webhook URL |

---

## 8. Build and Packaging Notes

### Build Configuration

```xml
<TargetFramework>net8.0-windows</TargetFramework>
<UseWindowsForms>true</UseWindowsForms>
<PublishSingleFile>true</PublishSingleFile>
<SelfContained>true</SelfContained>
<RuntimeIdentifier>win-x64</RuntimeIdentifier>
```

### Release Trimming

- `PublishTrimmed = true` with `TrimMode = full` (Release only).
- `BuiltInComInteropSupport = true` is **required** — WinForms and TaskScheduler use COM.
- `TrimmerRoots.xml` preserves `System.Windows.Forms` and `System.Windows.Forms.Primitives` assemblies in full to protect internal COM interface types.
- `TrimmerSingleWarn = false` expands IL2104 into per-member warnings for review.

### Embedded Resources

| Resource | Logical Name | Purpose |
|----------|-------------|---------|
| `Assets/jci_globe.ico` | `ProcDumpMonitor.jci_globe.ico` | Window/taskbar icon |
| `Assets/jci_globe_256.png` | `ProcDumpMonitor.jci_globe_256.png` | About page logo |

### NuGet Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `MailKit` | 4.7.1.1 | SMTP email sending |
| `System.ServiceProcess.ServiceController` | 8.0.1 | Enumerate Windows services for target selection |
| `TaskScheduler` | 2.12.2 | Windows Task Scheduler COM wrapper |

---

## 9. How to Review This Code

### Suggested Entry Points

1. **`Program.cs`** — Read the CLI dispatcher first to understand all operating modes.
2. **`MainForm.cs`** — Follow the wizard page array and `NavigateTo()` to see the UI flow.
3. **`Config.cs`** — Understand the data model; `BuildProcDumpArgs()` shows how options map to ProcDump flags.
4. **`ProcDumpMonitor.cs` (`ProcDumpMonitorLoop.Run`)** — The core monitoring loop; follow the cycle to see how ProcDump is launched, dumps detected, and notifications sent.
5. **`TaskSchedulerService.cs`** — How the task is registered (SYSTEM principal, BootTrigger, restart policy).

### Critical Paths

| Path | Files | Why It Matters |
|------|-------|----------------|
| **Config round-trip** | `Config.cs` → `ConfigMigrator.cs` → `ConfigExportImport.cs` | Data loss or migration bugs break all deployments |
| **Task registration** | `TaskSchedulerService.InstallOrUpdate()` | Incorrect principal or trigger = silent monitoring failure |
| **Credential flow** | `Config.SetPassword()` → `Config.GetPassword()` → `EmailNotifier` | DPAPI scope and entropy must match across encrypt/decrypt |
| **Dump detection** | `ProcDumpMonitorLoop` → `DumpStabilityChecker` → `NotificationQueue` | Race between partial dump write and notification send |
| **Disk guard** | `DiskSpaceGuard.Check()` called before each ProcDump launch | Prevents filling the disk, which could crash the monitored service |

### Quick Validation

```bash
# Build
dotnet build

# Run tests
dotnet test tests/ProcDumpMonitor.Tests/

# Publish single-file (Release)
dotnet publish -c Release -r win-x64

# Smoke test (no target needed)
ProcDumpMonitor.exe --selftest
```

---

*End of report.*

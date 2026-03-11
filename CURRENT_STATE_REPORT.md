# ProcDump Monitor -- Current-State Codebase Report

Generated from the `main` branch at commit time.
This document describes the code **as it exists now**, not historical intent.

---

## 1. Executive Summary

ProcDump Monitor is a Windows desktop application (.NET 8, WinForms) that automates crash-dump collection using Microsoft's [ProcDump](https://learn.microsoft.com/en-us/sysinternals/downloads/procdump) utility. It provides:

- A **wizard-style GUI** for configuring a target process, ProcDump flags, a Windows Scheduled Task, and email/webhook notifications.
- A **headless monitor mode** (`--monitor`) that runs ProcDump in an infinite restart loop, detects new `.dmp` files, checks dump stability, applies retention policies, guards disk space, and sends notifications.
- A **one-shot mode** (`--oneshot`) that creates a scheduled task, captures (or simulates) a dump, sends a notification, removes the task, and exits.
- **CLI verbs** for task management (`--install`, `--uninstall`, `--start`, `--stop`, `--status`), config export, self-test, and version queries.

**Deployment model:** Copy the published single-file EXE (plus `procdump.exe`/`procdump64.exe`) to a folder on the target server. Run the GUI to configure, then let the scheduled task run headless under SYSTEM. No installer, no MSI, no service registration.

**Privilege assumptions:** Creating/modifying a Windows Scheduled Task that runs as SYSTEM at highest run level requires Administrator privileges. The GUI warns if not elevated and offers a UAC relaunch button. The monitor loop itself does not require elevation once the task is running under SYSTEM.

---

## 2. Build and Publish Model

### Project file

`ProcDumpMonitor.csproj` -- SDK-style, `OutputType=WinExe`, targets `net8.0-windows` with `UseWindowsForms=true`.

### Debug build

```
dotnet build
```

Produces a standard multi-file output under `bin\Debug\net8.0-windows\win-x64\`. IL trimming is **not** applied in Debug (`PublishTrimmed` is only set in the `Release` condition group), so F5 debugging works without reflection or COM issues.

### Publish (Release)

```
dotnet publish -c Release
```

Or via the publish profile:

```
dotnet publish -c Release /p:PublishProfile=Properties\PublishProfiles\win-x64-singlefile.pubxml
```

**Publish profile** (`Properties\PublishProfiles\win-x64-singlefile.pubxml`): outputs to `bin\publish\`.

**Release-mode settings** (in `.csproj`):

| Setting | Value | Purpose |
|---|---|---|
| `PublishSingleFile` | `true` | Single EXE output |
| `SelfContained` | `true` | Ships the .NET runtime |
| `RuntimeIdentifier` | `win-x64` | Windows x64 only |
| `PublishTrimmed` | `true` (Release only) | IL linker removes unused code |
| `TrimMode` | `full` | Aggressive trimming |
| `IncludeNativeLibrariesForSelfExtract` | `true` | Native DLLs inside the bundle |
| `EnableCompressionInSingleFile` | `true` | Smaller EXE |
| `BuiltInComInteropSupport` | `true` | Required for WinForms COM interop |
| `SatelliteResourceLanguages` | `en` | English only; strips other locales |
| `_SuppressWinFormsTrimError` | `true` | Suppresses the WinForms trimming error |

**Trimmer safety** is handled by `TrimmerRoots.xml`, which preserves three assemblies in full:

1. `System.Windows.Forms` -- internal COM interfaces for OLE drag-drop, accessibility, etc.
2. `System.Windows.Forms.Primitives` -- companion COM types.
3. `Microsoft.Win32.TaskScheduler` -- COM import interfaces for the Task Scheduler V2 API.

Without these roots, the published EXE throws `TypeLoadException` or `E_INVALIDARG` at runtime.

### Assets folder

The `Assets\` folder **exists on disk** and contains:

| File | Size | Purpose |
|---|---|---|
| `jci_globe.ico` | ~97 KB | Application icon (window + taskbar) |
| `jci_globe_256.png` | ~23 KB | Not currently referenced in code |
| `logo192.png` | ~13 KB | Not currently referenced in code |
| `logo512.png` | ~57 KB | About page logo |

The `.ico` and `logo512.png` are compiled as **embedded resources** with explicit `LogicalName` attributes in the `.csproj`:

- `ProcDumpMonitor.jci_globe.ico`
- `ProcDumpMonitor.logo512.png`

They are **not** copied to the output directory. The published single-file EXE contains them inside the assembly. `jci_globe_256.png` and `logo192.png` are present on disk but **not referenced** by any code or project item.

### Output structure

A successful `dotnet publish -c Release` produces:

```
bin\publish\
  ProcDumpMonitor.exe    (single self-contained file, ~30-50 MB)
```

No other files are required. The user is expected to place `procdump.exe` or `procdump64.exe` alongside the EXE (or specify an absolute path in config).

---

## 3. Runtime Modes and Entry Points

All entry points go through `Program.Main(string[] args)` in `Program.cs`.

### GUI mode (no arguments)

```
ProcDumpMonitor.exe
```

Calls `ApplicationConfiguration.Initialize()` and `Application.Run(new MainForm())`. Launches the WinForms wizard. This is the default when double-clicking the EXE.

### CLI modes

All CLI modes skip WinForms initialization entirely. The `--config <path>` flag is optional for most verbs; if omitted, `Config.DefaultConfigPath` (i.e., `config.json` next to the EXE) is used.

| Flag | Behavior | Exit code |
|---|---|---|
| `--help`, `-h`, `/?` | Print usage text | 0 |
| `--version` | Print assembly version | 0 |
| `--selftest` | Run `OneShotSelfTest.Run()` with all fakes | 0 = pass, 1 = fail |
| `--monitor --config <path>` | Enter `ProcDumpMonitorLoop.Run(cfg)` -- infinite loop | Does not exit normally |
| `--oneshot [flags]` | Run `OneShotRunner.Execute()` | 0 = success, 1 = failure |
| `--install` / `--update` | `TaskSchedulerService.InstallOrUpdate(cfg)` | 0 / 1 |
| `--uninstall` | `TaskSchedulerService.RemoveTask(taskName)` | 0 / 1 |
| `--start` | `TaskSchedulerService.StartNow(taskName)` | 0 / 1 |
| `--stop` | `TaskSchedulerService.StopTask(taskName)` | 0 / 1 |
| `--status` | Print `CliStatusOutput` JSON to stdout | 0 / 1 |
| `--export-config <path>` | Export config with secrets redacted | 0 / 1 |
| Unknown | Print error to stderr | 2 |

### Monitor mode lifecycle

1. `ProcDumpMonitorLoop.Run(cfg)` configures logger rotation, loads previous `HealthStatus` from `health.json`.
2. Registers `Console.CancelKeyPress` to set `_stopping = true`.
3. Enters `while (!_stopping)` loop:
   a. Check disk space via `DiskSpaceGuard`. If below threshold, log, rate-limit a low-disk notification (once per hour), sleep, continue.
   b. Run `RetentionPolicy.Apply()` to delete aged/over-cap dumps.
   c. Launch ProcDump as a child process (`RunProcDumpCycle`).
   d. Poll `proc.WaitForExit(1000)` in a loop, writing a health heartbeat every 30 seconds.
   e. On exit, call `DetectAndNotify` to find the newest `.dmp` file created after cycle start.
   f. If a new dump is found, run `DumpStabilityChecker.WaitForStableFile()` (size-stable + exclusive file lock).
   g. If stable and not a duplicate, enqueue notifications via `NotificationQueue`.
   h. Write `HealthStatus` to `health.json`, sleep `RestartDelaySeconds`, restart.
4. On `_stopping`, log and return.

### Shutdown and cancellation

- **Monitor mode:** `Console.CancelKeyPress` sets `_stopping = true`. The sleep loop (`WaitBeforeRestart`) checks `_stopping` every 100 ms. If ProcDump is still running, it is killed via `proc.Kill(true)`.
- **One-shot mode:** A `CancellationTokenSource` is wired to `Console.CancelKeyPress`. The runner checks `ct.IsCancellationRequested` between steps and returns early with `Error = "Cancelled."`.
- **GUI:** Standard WinForms `Application.Exit()`. The tray icon and menu are disposed in `OnFormClosing`.

---

## 4. Canonical Path Model

All paths are resolved by the static class `AppPaths` (`AppPaths.cs`).

### Install directory resolution

```csharp
public static string InstallDir
```

Uses `Environment.ProcessPath` to get the true on-disk path to the running EXE, then calls `Path.GetDirectoryName()`. This correctly handles single-file publish, where `AppContext.BaseDirectory` may point to a temp extraction folder. Falls back to `AppContext.BaseDirectory` if `ProcessPath` is null (should not happen on .NET 8 Windows).

The result is cached in `_installDir` (a `static string?` field). Tests can override it via `AppPaths.SetInstallDir(string dir)` (internal).

### File locations

| File | Path | Purpose |
|---|---|---|
| `config.json` | `{InstallDir}\config.json` | User configuration |
| `health.json` | `{InstallDir}\health.json` | Monitor health heartbeat |
| `Logs\procdump.log` | `{InstallDir}\Logs\procdump.log` | Main log file |
| `Logs\procdump.log.1` ... `.N` | Same directory | Rotated log files |

### Behavior by deployment scenario

| Scenario | InstallDir resolves to | Notes |
|---|---|---|
| Portable folder copy | Folder containing `ProcDumpMonitor.exe` | All data files live alongside the EXE |
| Admin interactive run | Same as above | Works if user has write access to the folder |
| SYSTEM scheduled task | Same as above | SYSTEM has full access; DPAPI uses `LocalMachine` scope, so secrets encrypted by the admin user are decryptable |
| Single-file publish | Folder containing the bundle EXE (not the temp extraction dir) | `Environment.ProcessPath` is the key; `AppContext.BaseDirectory` is explicitly avoided as the primary source |

### Important caveat

If the EXE is placed in `C:\Program Files\...`, the SYSTEM account can write to it, but a non-admin interactive user may not be able to modify `config.json` or `Logs\`. The application does not create or modify ACLs.

---

## 5. Configuration Model

### Schema overview (`Config.cs`)

`Config` is a plain POCO serialized to JSON via source-generated `ConfigJsonContext`. Key property groups:

| Group | Properties |
|---|---|
| Schema | `ConfigVersion` (int, current = 2) |
| Target | `TargetName` |
| ProcDump core | `ProcDumpPath`, `DumpDirectory`, `DumpType`, `DumpOnException`, `DumpOnTerminate`, `UseClone`, `MaxDumps`, `RestartDelaySeconds` |
| Advanced triggers | `CpuThreshold`, `CpuLowThreshold`, `MemoryCommitMB`, `HangWindowSeconds` |
| Disk guard | `MinFreeDiskMB` (default 5120) |
| Dump stability | `DumpStabilityTimeoutSeconds` (default 30), `DumpStabilityPollSeconds` (default 2) |
| Log rotation | `MaxLogSizeMB` (default 10), `MaxLogFiles` (default 5) |
| Dump retention | `DumpRetentionDays` (0 = off), `DumpRetentionMaxGB` (0 = off) |
| Task | `TaskName` (default "ProcDump Monitor") |
| Email | `EmailEnabled`, `SmtpServer`, `SmtpPort`, `UseSsl`, `FromAddress`, `ToAddress`, `CcAddress`, `SmtpUsername`, `EncryptedPasswordBlob` |
| Webhook | `WebhookEnabled`, `WebhookUrl` |

### Versioning and migration (`ConfigMigrator.cs`)

- **V1 configs** (no `ConfigVersion` field, or `ConfigVersion == 0`): Upgraded to V2 automatically. New fields get their C# property-initializer defaults. A log entry is written.
- **V2 configs** (`ConfigVersion == 2`): Current version. No migration needed.
- **Future configs** (`ConfigVersion > 2`): Loaded as-is. `ConfigMigrator.DowngradeWarning` is set to `true`. `MainForm` shows a warning dialog on load.
- **Backup:** `Config.Save()` calls `ConfigMigrator.BackupIfNeeded(path)`, which copies the existing file to `config.json.bak` before overwriting.

### Secret storage

SMTP passwords are encrypted using **DPAPI** with `DataProtectionScope.LocalMachine` and a static entropy value (`"ProcDumpMonitor-SMTP-v1"`). The encrypted blob is stored as Base64 in `EncryptedPasswordBlob`.

- `Config.SetPassword(string plaintext)` encrypts and stores.
- `Config.GetPassword()` decrypts. Returns empty string on any failure.

### Serialization

Two source-generated JSON contexts:

1. `ConfigJsonContext` -- for `Config` only. `WriteIndented = true`, `DefaultIgnoreCondition = WhenWritingDefault`.
2. `AppJsonContext` -- for `HealthStatus`, `WebhookPayload`, `CliStatusOutput`, `OneShotResult`. `WriteIndented = true`, no ignore condition.

### Export/Import (`ConfigExportImport.cs`)

- **Export:** Deep-copies via serialize/deserialize, replaces `EncryptedPasswordBlob` and `WebhookUrl` with `<REDACTED>`, writes to file.
- **Import:** Loads via `ConfigMigrator.Migrate()`, forces `EmailEnabled = false` and `WebhookEnabled = false`, clears redaction markers.

---

## 6. ProcDump Execution and Monitoring Loop

### How ProcDump is launched (`ProcDumpMonitor.cs`, `RunProcDumpCycle`)

A `System.Diagnostics.Process` is created with:
- `FileName` = `cfg.ProcDumpPath`
- `Arguments` = `cfg.BuildProcDumpArgs()` (see below)
- `UseShellExecute = false`, `CreateNoWindow = true`
- `RedirectStandardOutput` and `RedirectStandardError` = `true`
- `WorkingDirectory` = `cfg.DumpDirectory`

Stdout and stderr are logged line-by-line via `OutputDataReceived`/`ErrorDataReceived` events.

### Argument construction (`Config.BuildProcDumpArgs`)

Builds a space-separated string: `-accepteula`, dump type flag (`-ma`/`-mp`/`-mm`), trigger flags (`-e`, `-t`, `-r`), advanced triggers (`-c N`, `-cl N`, `-m N`, `-h`), `-n {MaxDumps}`, `-w {TargetName}`, `"{DumpDirectory}"`.

The `-w` flag tells ProcDump to **wait** for the target process to appear.

### Restart behavior

ProcDump is launched once per cycle. After it exits (for any reason), the monitor loop:

1. Records the exit code in `HealthStatus`.
2. Runs dump detection and notification.
3. Sleeps for `RestartDelaySeconds` (default 5), checking `_stopping` every 100 ms.
4. Restarts the cycle.

There is no exponential backoff. The delay is fixed. If ProcDump exits immediately and repeatedly (e.g., invalid arguments), the loop will restart every `RestartDelaySeconds` indefinitely.

### Disk-space guard (`DiskSpaceGuard.cs`)

Checks `DriveInfo.AvailableFreeSpace` on the dump volume. If below `MinFreeDiskMB`, the cycle is skipped. A low-disk notification is rate-limited to once per hour. The guard **fails open** on errors (returns `true`), so monitoring is never blocked by a disk-check failure.

### Dump stability check (`DumpStabilityChecker.cs`)

Polls the file until:
1. File size is unchanged across two consecutive polls (`pollIntervalSeconds` apart, default 2s).
2. The file can be opened with `FileShare.None` (exclusive lock).

If the timeout (`DumpStabilityTimeoutSeconds`, default 30s) expires before both conditions are met, the dump is considered unstable and notification is suppressed. This prevents emailing about partial or corrupt dumps that ProcDump is still writing.

### Retention policy (`RetentionPolicy.cs`)

Two policies applied at the start of each cycle (before launching ProcDump):

1. **Age-based:** Delete `.dmp` files older than `DumpRetentionDays` days. 0 = disabled.
2. **Size-based:** If total `.dmp` size exceeds `DumpRetentionMaxGB`, delete oldest first until under cap. 0 = disabled.

Returns the count of deleted files. All errors are logged and swallowed.

### Guarantees and non-guarantees

- The loop **will** restart ProcDump after it exits, indefinitely, until signaled to stop.
- The loop **will not** detect dumps created outside its own ProcDump cycle (it filters by `LastWriteTimeUtc >= cycleStart`).
- The loop **does not** deduplicate across restarts beyond the `LastNotifiedDumpFile` field in `health.json`.
- There is **no watchdog** for the monitor process itself. If it crashes, the scheduled task's restart settings (1-minute interval, 999 retries) handle recovery.

---

## 7. Notification Pipeline

### Architecture

Two notifier implementations exist, both implementing `INotifier`:

1. `EmailNotifierAdapter` -- wraps the static `EmailNotifier` class.
2. `WebhookNotifier` -- posts JSON to a URL.

These are wired as a static array in `ProcDumpMonitorLoop`:

```csharp
private static readonly INotifier[] Notifiers = { new EmailNotifierAdapter(), new WebhookNotifier() };
```

### NotificationQueue (`NotificationQueue.cs`)

A `BlockingCollection<Action>` with a background `Thread`. Bounded to 64 items. The monitor loop enqueues notifications via `EnqueueDump` or `EnqueueWarning`, which iterate over all enabled notifiers and wrap each call in a try/catch. The queue processes items sequentially on a single background thread.

- If the queue is full, new items are dropped with a log warning.
- On dispose, `CompleteAdding()` is called and the worker is joined with a 5-second timeout.
- All exceptions inside the worker loop are caught and logged; they never propagate.

### Email (`EmailNotifier.cs`)

Uses **MailKit** (`SmtpClient`). Supports:

- Semicolon-delimited To and CC addresses.
- Three TLS modes: implicit SSL (port 465), STARTTLS (other ports with `UseSsl = true`), or `StartTlsWhenAvailable` (no SSL configured).
- Optional `NetworkCredential` authentication. If `SmtpUsername` is blank, no auth is attempted (anonymous relay).
- 30-second timeout.

Address validation (`ValidateAddressList`) uses `MailboxAddress.Parse` from MimeKit.

SMTP connectivity validation (`ValidateSmtpConnectivity`) performs a raw TCP connect and reads the SMTP banner. This is used by the "Validate SMTP" button in the UI.

### Webhook (`WebhookNotifier.cs`)

Posts a `WebhookPayload` (JSON) to the configured URL using a static `HttpClient` with a 15-second timeout. The payload format is compatible with Microsoft Teams `MessageCard` format (`@type`, `themeColor`, `title`, `text`). Slack will pick up the `text` field.

The HTTP call uses sync-over-async (`GetAwaiter().GetResult()`), which is acceptable because it runs on the `NotificationQueue` background thread. All HTTP and timeout exceptions are caught and logged.

### Error handling and health reporting

- Email and webhook failures are logged via `Logger.Log("NotifyQ", ...)`.
- Failures do **not** set `HealthStatus.LastError` -- that field is reserved for cycle-level errors.
- There is no retry mechanism for failed notifications. If an email fails, it is lost.

---

## 8. UI Architecture

### Wizard flow (`MainForm.cs`)

`MainForm` is the top-level `Form`. Layout is a 4-row `TableLayoutPanel`:

| Row | Content |
|---|---|
| 0 | Header (title label + elevation warning) |
| 1 | `StepIndicator` (owner-drawn numbered circles with connecting lines) |
| 2 | Content panel (fills remaining space; each wizard page is swapped in) |
| 3 | Footer (Export, Import, Back, Next buttons) |

The wizard has **6 pages**, each a `WizardPage` subclass (which extends `UserControl`):

| Index | Class | StepTitle | Responsibility |
|---|---|---|---|
| 0 | `TargetPage` | "Target" | Target process name; optional Windows service dropdown |
| 1 | `ProcDumpPage` | "ProcDump" | ProcDump path, dump directory, dump type, triggers, advanced triggers (collapsible), max dumps, restart delay, min free disk |
| 2 | `TaskPage` | "Task" | Scheduled task name, existing task detection badge, task action preview |
| 3 | `NotificationsPage` | "Notify" | Email config (SMTP, addresses, test/validate buttons); webhook URL; maintenance/retention settings (collapsible) |
| 4 | `ReviewPage` | "Review" | Summary text, action buttons (Create/Update Task, Run Now, Stop, Remove, One-Shot, Save Config Only, Open Dump Folder, View Logs, Copy ProcDump Cmd, Open Task Scheduler), status banner, log pane |
| 5 | `AboutPage` | "About" | Logo (from embedded resource), app name, author ("Matthew Raburn"), version/build date |

### Navigation

- `NavigateTo(index)` clears the content panel, adds the new page, calls `page.OnEnter(cfg)`, updates the step indicator, and wires `ValidationChanged`.
- "Next" calls `page.IsValid()` -- if false, marks the step as error in the indicator. If true, calls `page.OnLeave(cfg)` (which can return false to block navigation), marks as completed, and advances.
- "Back" calls `OnLeave(cfg)` (always succeeds) and goes to the previous page.
- On the final page (index 5, About), the Next button is hidden.

### Target selection (`TargetPage.cs`)

The target page offers two input methods:

1. **Manual text entry** in a `TextBox`.
2. **Windows service dropdown** (`ComboBox` with `DropDownStyle.DropDown`). Populated via `ServiceController.GetServices()`. A "Show all services (including stopped)" checkbox toggles the filter. Selecting a service copies its `ServiceName` to the text box.

The `ComboBox.TextChanged` event also handles manual typing in the dropdown, treating unrecognized text as a manual entry.

`ServiceController` instances are properly disposed in a `finally` block.

### Validation model

Each `WizardPage` implements `IsValid()`. The `ValidationChanged` event is raised whenever inputs change, and `MainForm` enables/disables the "Next" button accordingly. Validation labels (red text) are shown inline when fields are invalid.

- `TargetPage`: Process name must not be empty.
- `ProcDumpPage`: ProcDump path must exist on disk, dump directory must not be empty, at least one trigger must be active.
- `TaskPage`: Task name must not be empty.
- `NotificationsPage`: Always returns `true` from `IsValid()` (notifications are optional). Validated on `OnLeave` if email is enabled.
- `ReviewPage` / `AboutPage`: Always valid.

### Elevation check

`MainForm.CheckElevation()` checks `WindowsPrincipal.IsInRole(WindowsBuiltInRole.Administrator)`. If not elevated, shows an orange warning label. Clicking it calls `RelaunchElevated()`, which starts a new process with `Verb = "runas"` and exits the current one.

### Theme

`ThemeManager` applies a VS Code-style dark theme. Colors are defined as static fields. `ApplyTheme(Control)` recursively walks the control tree and sets `BackColor`, `ForeColor`, `FlatStyle`, etc. based on control type. Special cases:

- `ThemedCheckBox` is owner-drawn and skipped by the theme walker.
- `StepIndicator` is owner-drawn.
- Panels named `"BannerPanel"` or `"StatusBanner"` are skipped to preserve dynamic success/error colors.
- The Windows 10/11 dark title bar is enabled via `DwmSetWindowAttribute` P/Invoke.

### System tray

Minimizing the form hides it and shows a tray icon. Double-clicking the tray icon or selecting "Open" restores it.

### Config export/import

Footer buttons allow exporting (secrets redacted) and importing configs. Import forces notifications off and resets the wizard to page 0.

---

## 9. Diagnostics and Observability

### Logging (`Logger.cs`)

A static, thread-safe file logger. Writes to `{InstallDir}\Logs\procdump.log`.

- Format: `[yyyy-MM-dd HH:mm:ss] [{category}] {message}`
- **Log rotation:** When the file exceeds `MaxLogSizeMB` (default 10), it is rotated: `procdump.log` -> `procdump.log.1` -> `.2` -> ... -> `.{MaxLogFiles}`. The oldest file beyond `MaxLogFiles` (default 5) is deleted.
- All exceptions inside the logger are swallowed. Logging never crashes the monitor.
- There are no log levels (debug/info/warn/error). All messages are treated equally.

### health.json (`HealthWriter.cs`, `HealthStatus`)

Written atomically (temp file + `File.Move`) after each monitor cycle. Fields:

| Field | Type | Purpose |
|---|---|---|
| `MonitorPid` | int | PID of the monitor process |
| `ProcDumpPid` | int | PID of the current ProcDump child (0 between cycles) |
| `LastCycleUtc` | string (ISO 8601) | Timestamp of last cycle start (updated during wait as heartbeat) |
| `LastProcDumpExitCode` | int | Exit code from the most recent ProcDump run |
| `LastDumpFileName` | string | Name of the most recently detected dump |
| `TotalDumpCount` | int | Cumulative count (persists across monitor restarts) |
| `LastError` | string | Last cycle error message (cleared each cycle) |
| `NextRetryUtc` | string (ISO 8601) | When the next cycle will begin |
| `LastNotifiedDumpFile` | string | Used for deduplication |
| `LastNotifiedUtc` | string (ISO 8601) | Timestamp of last notification |
| `DiskSpaceLow` | bool | Whether disk space is below threshold |
| `FreeDiskMB` | long | Free disk space snapshot |
| `Version` | string | Assembly version |

An external monitoring tool can poll `health.json` to detect a stalled monitor (e.g., `LastCycleUtc` older than expected), low disk space, or repeated errors.

### Diagnostics bundle

There is **no built-in diagnostics bundle export** in the current codebase. A support engineer would need to manually collect:

1. `config.json` (contains `EncryptedPasswordBlob` -- DPAPI-protected, machine-scoped)
2. `health.json`
3. `Logs\procdump.log` and rotated files
4. Task Scheduler state via `--status`

There is **no automated system_info collection** or **redaction tool** beyond the config export feature.

### Build date

`BuildInfo.BuildDate` reads the `BuildDate` assembly metadata attribute, which is stamped at build time via MSBuild (`$([System.DateTime]::UtcNow.ToString("MM.dd.yy"))`). Displayed on the About page as "Version {date}".

---

## 10. Security Posture

### Privilege boundaries

- **GUI:** Runs as the interactive user. Can read/write config, browse files, and test email. Cannot create scheduled tasks without elevation.
- **Scheduled task:** Registered to run as `SYSTEM` with `RunLevel.Highest`. This gives the monitor full access to the local machine.
- **Monitor loop:** Inherits the privileges of the parent process (SYSTEM when run from the scheduled task).

### DPAPI usage

SMTP passwords are encrypted with `DataProtectionScope.LocalMachine`. This means:

- Any process running on the same machine can decrypt the password.
- The password is **not** portable between machines.
- If the machine is compromised, the password is exposed.
- This is a deliberate trade-off: the scheduled task runs as SYSTEM, which is a different user than the admin who configured the password. `LocalMachine` scope is required for cross-user decryption.

The entropy value (`"ProcDumpMonitor-SMTP-v1"`) is hardcoded in the source and provides no meaningful additional security beyond DPAPI's machine key.

### File system assumptions

- The application assumes write access to its own directory (for `config.json`, `health.json`, `Logs\`).
- No ACLs are created or modified.
- If the EXE is in a protected location (`Program Files`), the SYSTEM scheduled task can write, but interactive non-admin users cannot.

### Known risks

- SMTP password is recoverable by any admin on the machine (DPAPI `LocalMachine`).
- No TLS certificate validation override -- MailKit uses system defaults.
- The webhook URL is stored in plaintext in `config.json`. It is redacted in exports.
- ProcDump is launched with `CreateNoWindow = true` and `UseShellExecute = false`, which is correct.

---

## 11. Tests and Verification

### Test project

`tests\ProcDumpMonitor.Tests\ProcDumpMonitor.Tests.csproj` -- xUnit 2.9.3, Microsoft.NET.Test.Sdk 17.12.0, coverlet 6.0.4.

### Test files and coverage

| File | Tests | Covers |
|---|---|---|
| `ConfigTests.cs` | 3 tests | `ConfigMigrator` V1-to-V2 upgrade; downgrade warning; export/import round-trip with secret redaction |
| `ValidationTests.cs` | 6 tests | `TaskNameHelper.Sanitize` (edge cases, whitespace collapsing, invalid chars, slashes); `Config.BuildProcDumpArgs` (flag inclusion, CPU-zero omission); `EmailNotifier.ValidateAddressList` |
| `OneShotRunnerTests.cs` | 4 tests | `OneShotRunner` full flow with fakes (create/remove task, dump-then-notify-then-cleanup, email skip with `--no-email`, thread leak detection) |
| `RetentionPolicyTests.cs` | 4 tests | `RetentionPolicy` age-based and size-based deletion; `DiskSpaceGuard` fail-open and disabled behavior |

Total: **17 unit tests**, all using fakes/simulation (no real Task Scheduler, SMTP, or ProcDump).

### Known gaps

- No tests for `ProcDumpMonitorLoop` (the infinite monitor loop).
- No tests for `EmailNotifier.Send` (requires real SMTP).
- No tests for `WebhookNotifier.Post` (requires real HTTP endpoint).
- No tests for `NotificationQueue` (threading behavior).
- No tests for `DumpStabilityChecker` (requires file locking).
- No UI tests (WinForms wizard pages, theme application).
- No integration tests against real Task Scheduler.
- `Config.Save`/`Config.Load` are not directly tested in isolation (tested indirectly through export/import).
- `Logger` rotation is not tested.

### Build warnings

- Trimming warnings are enabled (`TrimmerSingleWarn=false`, `SuppressTrimAnalysisWarnings=false`). Any IL2xxx warnings will surface during `dotnet publish -c Release`.
- `_SuppressWinFormsTrimError=true` suppresses the blanket "WinForms is not trim-compatible" error.

---

## 12. Known Sharp Edges and Non-Goals

### Behaviors that may surprise a maintainer

1. **No backoff on ProcDump failure.** If ProcDump exits immediately (e.g., bad path, invalid args), the monitor restarts it every `RestartDelaySeconds`. This can produce rapid log growth.

2. **TaskNameHelper collapses ` - ` to a single space.** The regex `[\s\-]{2,}` treats a run of any mix of whitespace and dashes as collapsible. So `"ProcDump Monitor - MyApp"` becomes `"ProcDump Monitor MyApp"`. This is intentional but may surprise users who expect the dash.

3. **NotificationsPage hosts maintenance settings.** Log rotation, dump retention, and dump stability timeout are configured on the "Notify" page under a collapsible "Maintenance & Retention" section. This is a UI layout decision, not a logical grouping.

4. **ReviewPage's One-Shot button uses `SimulatedProcDumpRunner` with `RealTaskSchedulerOps`.** This creates a real scheduled task, simulates the dump, sends a real email, then removes the task. It is not a pure simulation.

5. **`_health` in `ProcDumpMonitorLoop` is a static field.** The monitor loop is designed to run exactly once per process lifetime. Running it twice in the same process would share state.

6. **Webhook sync-over-async.** `WebhookNotifier.Post` calls `Http.PostAsync(...).GetAwaiter().GetResult()`. This is safe because it runs on the `NotificationQueue` background thread, but it would deadlock if called from a UI thread.

7. **`ServiceItem` is defined in `MainForm.cs`** (at the bottom), not in its own file or in `TargetPage.cs`.

8. **`ValidationException` is defined in `MainForm.cs`** but does not appear to be used anywhere in the current codebase.

9. **Two unused asset files** (`jci_globe_256.png`, `logo192.png`) exist in the `Assets\` folder but are not referenced.

### Things intentionally not supported

- **Linux/macOS.** WinForms, Task Scheduler, and DPAPI are Windows-only.
- **Multiple simultaneous targets.** One config, one target, one scheduled task.
- **Remote management.** No REST API, no remote config push.
- **Dump file attachment in email.** Notifications contain the file path, not the file itself (dumps are often multi-GB).
- **Encrypted config at rest** (beyond the SMTP password blob).
- **Automatic ProcDump download or update.**

### Areas requiring care if modified

- **TrimmerRoots.xml:** Removing any of the three preserved assemblies will break the published build. Any new NuGet package using COM interop may need to be added.
- **Source-generated JSON contexts:** Adding new types to serialize requires adding `[JsonSerializable(typeof(T))]` to the appropriate context class. Reflection-based `JsonSerializer` calls will fail in trimmed builds.
- **AppPaths.InstallDir caching:** The install dir is computed once and cached. If the EXE is moved at runtime (unlikely but possible), the cache is stale.
- **DPAPI entropy string:** Changing `"ProcDumpMonitor-SMTP-v1"` will make all existing encrypted passwords unreadable.
- **Config version:** Adding new config fields requires incrementing `Config.CurrentVersion` and adding migration logic in `ConfigMigrator.Migrate`.

---

## 13. Recommended Next Improvements

### High ROI (low effort, high impact)

1. **Add exponential backoff to the monitor loop.** If ProcDump exits immediately N times in a row, increase the delay geometrically (e.g., 5s, 10s, 30s, 60s, cap at 5 minutes). This prevents log flooding and CPU waste when ProcDump is misconfigured.

2. **Add a `--diag` CLI command** that collects `config.json` (with redacted secrets), `health.json`, recent log files, OS version, .NET version, and Task Scheduler state into a single ZIP file. This would significantly reduce support burden.

3. **Add notification retry with limited attempts.** Currently, a failed email is permanently lost. A simple 2-retry with 10-second delay in the `NotificationQueue` worker would improve reliability.

4. **Remove unused `ValidationException` class** from `MainForm.cs`.

### Medium effort

5. **Unit-test `NotificationQueue`** (bounded capacity, drop behavior, dispose/drain).

6. **Unit-test `Logger` rotation** (file size threshold, numbered file shifting, oldest deletion).

7. **Add structured log levels** (Debug, Info, Warn, Error) so that verbose ProcDump output can be filtered without discarding error messages.

8. **Move `ServiceItem` to its own file** (or into `TargetPage.cs`) for discoverability.

9. **Clean up unused asset files** (`jci_globe_256.png`, `logo192.png`) or add them as embedded resources if they are intended for future use.

### Longer-term

10. **Evaluate replacing the static `ProcDumpMonitorLoop` with an instance-based design** to improve testability and eliminate shared static state.

11. **Consider a lightweight health endpoint** (e.g., named pipe or local HTTP) so that monitoring tools do not need to poll a file.

12. **Document the `--selftest` output format** so it can be consumed by CI pipelines.

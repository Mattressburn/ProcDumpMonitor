# Post-Change Report: Support Diagnostics Feature

## 1. Summary of Change

Added a standalone **Support Diagnostics** feature to ProcDump Monitor that collects system information, application artifacts, Windows Event Viewer logs, and CrossFire logs into a single ZIP bundle. The feature is completely independent of the ProcDump configuration wizard. It can run without a configured target, without ProcDump installed, and without a scheduled task.

All invocation paths converge on one central method: `SupportDiagnosticsService.CreateBundle()`.

---

## 2. Invocation Paths

### Primary owners

| Entry Point | Location | Trigger | Elevation Behavior |
|---|---|---|---|
| **Tools menu** | `MainForm` menu bar | `Tools > Support Diagnostics...` | Prompts for UAC relaunch if not elevated |
| **System tray** | Tray icon right-click menu | `Create Support Bundle` | Prompts for UAC relaunch if not elevated |
| **CLI** | `Program.Main` | `--support-diagnostics [--since <ISO8601>] [--until <ISO8601>]` | Prints error to stderr and exits 1 if not elevated |

### Convenience shortcut

| Entry Point | Location | Trigger | Notes |
|---|---|---|---|
| **ReviewPage** | Utilities row on wizard step 5 | `Support Diagnostics...` button | Calls `MainForm.RunSupportDiagnosticsFromGui()`. No wizard-state assumptions. No special-cased elevation logic. Tooltip: "Shortcut to Support Diagnostics. This feature is available at any time from Tools or the tray menu." |

### Diagnostics Ownership

Diagnostics are **not** part of the wizard flow. They do not depend on wizard progress, a configured target, or any other page state. The three primary owners (Tools menu, tray, CLI) are accessible at all times, regardless of which wizard page is active or whether the wizard has been started at all.

The ReviewPage button exists only as a convenience shortcut. It delegates directly to `MainForm.RunSupportDiagnosticsFromGui()` via `FindForm()`, following the exact same code path as the menu and tray entry points. It has no custom elevation logic, no wizard-state gating, and no independent error handling.

### CLI usage

```
ProcDumpMonitor.exe --support-diagnostics
ProcDumpMonitor.exe --support-diagnostics --since "2025-01-20T00:00:00Z" --until "2025-01-21T00:00:00Z"
```

Default time range: last 24 hours.

---

## 3. Elevation Enforcement

### GUI (menu, tray, and ReviewPage shortcut)

All GUI entry points follow the same code path through `MainForm.RunSupportDiagnosticsFromGui()`:

1. `SupportDiagnosticsService.IsElevated()` checks `WindowsPrincipal.IsInRole(WindowsBuiltInRole.Administrator)`.
2. If not elevated, a `MessageBox` asks whether to relaunch elevated.
3. On confirmation, `SupportDiagnosticsService.RelaunchElevatedForDiagnostics()` starts a new process with `Verb = "runas"` and `--support-diagnostics` arguments.
4. The current (non-elevated) instance exits via `Application.Exit()`.
5. The elevated instance runs the CLI code path, collects the bundle, and exits.

If the user cancels UAC, no action is taken. The ReviewPage shortcut does not implement its own elevation handling.

### CLI

Prints to stderr and exits with code 1:
```
ERROR: --support-diagnostics requires Administrator privileges.
Re-run this command from an elevated (Administrator) prompt.
```

---

## 4. Output Location Rules

The ZIP output directory is resolved in order of preference:

1. `Config.DumpDirectory` - if `Config.Load()` succeeds and the directory exists on disk.
2. `AppPaths.InstallDir` - the folder containing the EXE.
3. `Path.GetTempPath()` - fallback if neither above is writable or valid.

Each fallback is wrapped in try/catch. The bundle never fails solely because of a missing or invalid configuration.

### File naming

```
ProcDumpMonitor_SupportBundle_<HOSTNAME>_<yyyyMMdd_HHmmss>.zip
```

Example: `ProcDumpMonitor_SupportBundle_SERVER01_20250124_143022.zip`

---

## 5. Bundle Contents

### E1: ProcDump Monitor Artifacts (if present)

| Item | Source | Archive Path |
|---|---|---|
| Log files | `AppPaths.LogDir` (all files including rotated `.1`, `.2`, …) | `Logs/` |
| health.json | `AppPaths.HealthPath` | `health.json` |
| Redacted config | `ConfigExportImport.Export()` (secrets replaced with `<REDACTED>`) | `config_redacted.json` |
| Task status | `TaskSchedulerService.GetDetailedStatus()` serialized via `AppJsonContext` | `status.json` |

All items are optional. Missing files or failed queries are noted in the manifest.

### E2: System Information

Generates `system_info.json` (machine-readable) and `system_info.txt` (human-readable).

| Field | API | Trim-safe |
|---|---|---|
| Host Name | `Environment.MachineName` | ✓ |
| OS Version | `RuntimeInformation.OSDescription` | ✓ |
| System Type | `RuntimeInformation.OSDescription` + `ProcessArchitecture` | ✓ |
| CPU | `PROCESSOR_IDENTIFIER` env var + `Environment.ProcessorCount` | ✓ |
| Boot Time | `DateTime.UtcNow - Environment.TickCount64` | ✓ |
| Memory | P/Invoke `GlobalMemoryStatusEx` (kernel32) | ✓ |
| Free Space | `DriveInfo.GetDrives()` for fixed drives | ✓ |
| Network (IPs, MACs, gateway, DNS, DHCP, subnet) | `NetworkInterface.GetAllNetworkInterfaces()` | ✓ |
| Logon Domain / Server | `Environment.UserDomainName`, `LOGONSERVER` env var | ✓ |
| Machine Domain | `IPGlobalProperties.GetIPGlobalProperties().DomainName` | ✓ |
| CCURE Version | Registry lookup: `HKLM\SOFTWARE\Tyco\CCURE 9000` and JCI variants | ✓ |

No WMI is used. All calls return `"N/A"` on failure.

### E3: Event Viewer Logs

| Item | Method | Archive Path |
|---|---|---|
| Application.evtx | `wevtutil epl` with time-bounded XPath query | `EventLogs/Application.evtx` |
| System.evtx | `wevtutil epl` with time-bounded XPath query | `EventLogs/System.evtx` |
| event_summary.txt | `wevtutil qe /f:text` parsed for error/warning counts + top 20 newest errors | `EventLogs/event_summary.txt` |

Time range defaults to the last 24 hours. Overridable via `--since`/`--until`.

### E4: CrossFire Logs

Searches:
- `C:\Program Files (x86)\Tyco\CrossFire\Logging`
- `C:\Program Files (x86)\JCI\CrossFire\Logging`

Rules:
- If one exists, its contents are included under `CrossFire/<Vendor>/`.
- If both exist, both are included in separate subdirectories.
- Only files modified within the selected time range are copied.
- **500 MB safety cap** per vendor root. Newest files first. A `_TRUNCATED.txt` note is written if capped.
- If neither path exists, a `_NOT_FOUND.txt` note is included.

### E5: Manifest

`bundle_manifest.json` contains:
- Generation timestamp, hostname, app version.
- Each collected source with archive path and bytes copied.
- All errors encountered during collection.

---

## 6. Files Changed

### New file

| File | Purpose |
|---|---|
| `SupportDiagnosticsService.cs` | Core diagnostics service. Contains `SupportDiagnosticsService` (static class with `CreateBundle`, `IsElevated`, `RelaunchElevatedForDiagnostics`), POCO models (`BundleManifest`, `BundleManifestEntry`, `SystemInfoData`, `DiskSpaceEntry`), the `DiagnosticsProgress` delegate, and `DiagJsonContext` (source-generated JSON context for trim-safe serialization). |

### Modified files

| File | Change |
|---|---|
| `Program.cs` | Added `--support-diagnostics` CLI verb with `--since`/`--until` argument parsing. Added `Diagnostics:` section to `PrintHelp()`. |
| `MainForm.cs` | Added `MenuStrip _menuStrip` field. Added `BuildMenu()` method creating `Tools > Support Diagnostics...`. Added `RunSupportDiagnosticsFromGui()` (`internal async void`) with elevation check, UAC relaunch, wait cursor, and result dialog. Added `"Create Support Bundle"` item to tray context menu. Removed redundant explicit `ThemeManager.ApplyTheme(_menuStrip)` call (now handled automatically by the control-tree walk). |
| `Pages\ReviewPage.cs` | Moved `_btnSupportDiag` out of the main action button row into a separate "Utilities" section with reduced visual prominence (smaller font, italic label, smaller `MinimumSize`). Added `ToolTip` stating the button is a shortcut. Click handler delegates to `MainForm.RunSupportDiagnosticsFromGui()` via `FindForm()` instead of implementing independent elevation or bundle logic. |
| `ThemeManager.cs` | Fixed MenuStrip / ToolStrip dark-mode theming. See "MenuStrip Theming" below. |

### MenuStrip Theming

The `Tools` menu text was nearly invisible in dark mode because `ApplyTheme(ToolStrip)` only set colors on top-level items and did not recurse into dropdown submenus. The `DarkColorTable` also lacked overrides for many `ProfessionalColorTable` color slots, causing the WinForms renderer to fall back to system defaults (light backgrounds, dark borders).

Changes made inside `ThemeManager`:

1. **Recursive item theming.** `ApplyTheme(ToolStrip)` now calls a private `ApplyToolStripItems()` helper that walks `ToolStripMenuItem.DropDownItems` recursively. Each dropdown gets its own `BackColor`/`ForeColor` set explicitly so nested submenus render correctly.

2. **Automatic discovery.** `ApplyRecursive` now includes a `case ToolStrip ts:` branch. Any `MenuStrip` or `ToolStrip` found in the form's control tree is themed during the standard `ApplyTheme(Control)` walk. Explicit per-strip calls are only needed for strips not in the control tree (e.g., the tray `ContextMenuStrip`).

3. **Complete `DarkColorTable`.** All `ProfessionalColorTable` virtual properties are now overridden:
   - Menu strip background: `PanelBackground` (#252526)
   - Hover/selection: `Accent` (#0E639C)
   - Pressed state on top-level items: `PanelBackground` (prevents bright flash when opening a dropdown)
   - Dropdown background, image margin gutter: `PanelBackground`
   - Check mark backgrounds: `Accent` with lightened/darkened variants for hover/press
   - ToolStrip panel, status strip, content panel, overflow, grip, button states, separators, borders, rafting container: all mapped to the dark palette

4. **`RoundedEdges = false`** on the renderer to avoid light-colored rounded-corner artifacts that become visible against a dark background.

The tray `ContextMenuStrip` (`_trayMenu`) is not part of the form's control tree, so it retains its explicit `ThemeManager.ApplyTheme(_trayMenu)` call. This is correct and unchanged.

---

## 7. Serialization and Trim Safety

A new source-generated JSON context was added:

```csharp
[JsonSourceGenerationOptions(WriteIndented = true)]
[JsonSerializable(typeof(BundleManifest))]
[JsonSerializable(typeof(SystemInfoData))]
internal partial class DiagJsonContext : JsonSerializerContext { }
```

This follows the existing pattern (`ConfigJsonContext`, `AppJsonContext`) and ensures all serialization works in trimmed single-file builds. No reflection-based `JsonSerializer` calls were added.

The only new native interop is `GlobalMemoryStatusEx` from `kernel32.dll` for memory information. This is always available on Windows.

---

## 8. Risks and Limitations

| Risk | Severity | Mitigation |
|---|---|---|
| `wevtutil` unavailable or blocked | Low | Errors caught and noted in manifest; other sections still collected |
| Event summary text parsing assumes English locale | Medium | Field names (`Level:`, `Date:`, `Source:`, `Event ID:`) are consistent on English Windows; may need XML format (`/f:xml`) for localized OS |
| Large CrossFire log directories | Low | 500 MB cap per vendor root with newest-first ordering and `_TRUNCATED.txt` note |
| ZIP creation uses disk space in temp directory | Low | Temp directory cleaned up in `finally` block |
| GUI elevation relaunch exits current process | By design | Matches existing elevation behavior for scheduled task operations |
| No cancellation support during bundle creation | Low | Bundle creation is typically fast (seconds); worst case is event log export taking up to 30 seconds |
| ReviewPage shortcut depends on `FindForm()` cast | Low | Returns `null` (no-op) if the parent form is not `MainForm`; only possible in unusual test harnesses |

---

## 9. Recommended Follow-Up Improvements

1. **Progress dialog for GUI invocations.** Currently uses `Cursor.WaitCursor` + `Enabled = false`. A proper modal dialog with a progress bar and cancel button would improve UX for large bundles.

2. **`--diag-output <path>` CLI flag.** Allow overriding the output directory from the command line.

3. **XML-based event summary parsing.** Replace the text-based `wevtutil qe /f:text` parsing with `/f:xml` to avoid locale-dependent field name matching.

4. **Unit tests for `SupportDiagnosticsService`.** The core collection logic can be tested using temp directories and verifying ZIP contents and manifest structure.

5. **File-size metadata in manifest.** Add original vs. compressed size per entry for support triage.

6. **Cancellation token plumbing.** Pass a `CancellationToken` through `CreateBundle` so the GUI can allow cancellation during long-running event log exports.

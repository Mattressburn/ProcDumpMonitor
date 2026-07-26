# LogDump

A Windows utility that configures **Sysinternals ProcDump** as an unattended **Scheduled Task** for crash-dump monitoring, with optional **email and webhook notifications**.

> **A SWH L3 Production** — packaged for C•CURE deployments.

---

## Quick Start

1. Copy the published `LogDump.exe` and `procdump64.exe` into the same folder on the target machine.
2. Run `LogDump.exe` — it will request Administrator privileges automatically.
3. Walk through the six-step wizard described below.
4. On the **Review** page, click **Create Task** then **Run Task Now**.

> Single self-contained EXE (~2 MB) — no runtime to install, nothing else to copy besides `procdump64.exe`.

```
C:\Tools\LogDump\
├── LogDump.exe               ← single-file self-contained EXE
├── procdump64.exe            ← Sysinternals ProcDump (place beside the EXE)
├── config.json               ← auto-created on first save
├── health.json               ← heartbeat file written each monitor cycle
└── Logs\
    └── procdump.log          ← auto-created by monitor mode
```

---

## Requirements

- Windows 10/11 or Windows Server 2016+
- Sysinternals ProcDump (`procdump64.exe` and/or `procdump.exe`)
- **Administrator privileges** (required to create Scheduled Tasks and attach to processes; requested automatically via the EXE's embedded manifest)
- Rust (MSVC toolchain) — only if building from source

---

## Wizard Pages — Field-by-Field Reference

The GUI is a six-step wizard. Use **Next →** and **← Back** to navigate.

---

### Step 1 — Target

Choose the process that ProcDump will monitor.

| Field | Description |
|-------|-------------|
| **Process Name** | The Windows process name **without `.exe`** (e.g. `CrossFireService`, `notepad`). Type it directly, or pick from the service dropdown below and it fills in automatically. |
| **Select Service** | Dropdown listing Windows services on this machine. By default only **Running** services are shown. |
| **Show all services** | Check this to include Stopped/Disabled services in the dropdown. |
| **Refresh Services** | Re-enumerate services (useful if a service started since the wizard opened). |

**What happens:** When you pick a service, its internal service name (not the display name) is copied into the Process Name field. That name is what ProcDump uses with the `-w` (wait) flag.

> **Tip:** The process name also seeds the auto-generated Scheduled Task name on the Task page.

---

### Step 2 — ProcDump

Configure *how* ProcDump captures dumps. For most use cases, pick a **Scenario** and leave everything else alone.

#### Scenario dropdown

Presets that configure all the options below in one click. The default is **Crash capture**.

| Scenario | ProcDump flags | When to use |
|----------|---------------|-------------|
| **Crash capture** *(default)* | `-ma -e -t` | Process crashes with an unhandled exception or terminates unexpectedly. Standard post-mortem investigation. |
| **Hang capture** | `-ma -h` | Process window stops responding (hung). Diagnose UI freezes and deadlocks. |
| **High CPU spike capture** | `-ma -c 90 -s 10 -n 3` | CPU exceeds 90 % for 10+ seconds. Captures up to 3 dumps. Identify runaway threads. |
| **Memory threshold capture** | `-ma -m 2048 -n 3` | Memory commit exceeds 2048 MB. Captures up to 3 dumps. Investigate memory leaks. |
| **Low impact full dump** | `-a -r -ma` | One-time full dump using process cloning (`-r`). Process suspended for milliseconds, not the full write duration. |
| **Custom** | *(manual)* | You configure every option yourself. The wizard switches to this automatically if you change any individual option. |

> **Note:** If you manually change any option, the dropdown automatically switches to "Custom". Use the dropdown to return to a preset.

#### Effective command

Read-only preview showing exactly what ProcDump command line will be generated.

#### Target bitness

Auto-detected. The wizard checks whether the target process is 32-bit or 64-bit and selects the correct ProcDump binary (`procdump.exe` vs `procdump64.exe`). If a mismatch is detected, a warning appears.

#### Paths

| Field | Description |
|-------|-------------|
| **ProcDump Path** | Full path to `procdump64.exe` or `procdump.exe`. Auto-detected if placed next to the EXE. Use **Browse…** to override. |
| **Dump Directory** | Folder where `.dmp` files will be written. Must be writable by SYSTEM. Recommended: `C:\Dumps\<AppName>\`. |

#### Dump Type

| Value | ProcDump flag | Description |
|-------|--------------|-------------|
| **Full** | `-ma` | Complete memory dump — all process memory. Largest but most useful. |
| **MiniPlus** | `-mp` | Private memory regions only. Smaller than Full. |
| **Mini** | `-mm` | Thread stacks only. Very small, limited usefulness. |
| **ThreadDump** | `-mt` | Text-based thread stack dump. No binary data. |

#### Triggers

| Checkbox | ProcDump flag | Description |
|----------|--------------|-------------|
| **Dump on unhandled exception** | `-e` | Capture when the process throws an exception that reaches the OS. |
| **Dump on hung window** | `-h` | Capture when a window stops responding. |
| **Dump on terminate** | `-t` | Capture when the process exits (regardless of reason). |

#### CPU Options

| Field | ProcDump flag | Description |
|-------|--------------|-------------|
| **CPU % (-c)** | `-c <N>` | Trigger dump when CPU usage is **above** this percentage. `0` = disabled. |
| **CPU Low % (-cl)** | `-cl <N>` | Trigger dump when CPU usage drops **below** this percentage. `0` = disabled. |
| **Duration sec (-s)** | `-s <N>` | CPU must exceed the threshold for this many consecutive seconds before triggering. |
| **Count (-n)** | `-n <N>` | Maximum number of dumps before ProcDump exits this cycle. |
| **Per-CPU threshold (-u)** | `-u` | Apply the threshold per logical CPU core instead of total. |

#### Memory

| Field | ProcDump flag | Description |
|-------|--------------|-------------|
| **Commit threshold MB (-m)** | `-m <N>` | Trigger dump when process private memory commit exceeds this value. `0` = disabled. |

#### Operational

| Checkbox | ProcDump flag | Description |
|----------|--------------|-------------|
| **Use clone / reflect (-r)** | `-r` | Capture dump via process cloning. The target is suspended for only milliseconds. Recommended for production services. |
| **Avoid outage (-a)** | `-a` | ProcDump exits if triggers fire too rapidly, preventing dump floods. |
| **Overwrite existing dump files (-o)** | `-o` | Overwrite instead of creating new numbered files. |
| **Wait for process to launch (-w)** | `-w` | ProcDump waits indefinitely for the target process to start. **Leave checked** for service monitoring. |
| **Accept EULA (-accepteula)** | `-accepteula` | Always on. Required by ProcDump to skip the EULA dialog. |

#### Numeric settings

| Field | Description |
|-------|-------------|
| **Restart delay (s)** | Seconds to wait after ProcDump exits before restarting the monitoring loop. Default: `5`. |
| **Min Free Disk (MB)** | Minimum free disk space required before ProcDump launches. If free space is below this threshold, the cycle is skipped. Default: `5120` (5 GB). |

#### Advanced Options *(collapsed by default)*

Most users should leave these empty. Click the **▶ Advanced Options** toggle to expand.

| Field | ProcDump flag | Description |
|-------|--------------|-------------|
| **Above threshold (-p)** | `-p <counter>` | Performance counter trigger. Dump when counter exceeds a value. |
| **Below threshold (-pl)** | `-pl <counter>` | Performance counter trigger. Dump when counter drops below a value. |
| **Include filter (-f)** | `-f <filter>` | Only capture dumps for exceptions whose name matches this filter (e.g. `OutOfMemory`). |
| **Exclude filter (-fx)** | `-fx <filter>` | Skip dumps for exceptions matching this filter (e.g. `ThreadAbort`). |
| **Register as WER debugger (-wer)** | `-wer` | Register ProcDump as the Windows Error Reporting post-mortem debugger. Only needed when `-e` is not catching crashes because WER intercepts them first. |
| **Avoid-terminate timeout (-at)** | `-at <N>` | Seconds. For rare edge cases where ProcDump blocks process shutdown. Leave at `0`. |

---

### Step 3 — Task

Configure the Windows Scheduled Task that runs the monitor loop.

| Field | Description |
|-------|-------------|
| **Scheduled Task Name** | Auto-generated as `LogDump <ProcessName>` from the target you selected in Step 1. You can edit it freely. |
| **Reset to Auto** | Button that regenerates the task name from the current target. Use this after manually editing if you want the auto-name back. |

The page also shows:

- **Detection badge** — Whether a task with this name already exists. If it does, the wizard will **update** it instead of creating a new one.
- **Existing Task Details** — State, last run time, last result, and next run time (visible only when the task exists).
- **Task Action Preview** — The exact command line the scheduled task will execute. Use **Copy Command** to copy it.

**Task properties (not editable in the UI — hardcoded for reliability):**

| Property | Value |
|----------|-------|
| Run as | `SYSTEM` (highest privileges) |
| Trigger | At system startup (BootTrigger) |
| Restart on failure | 1-minute interval, 999 retries |
| Multiple instances | Ignore new (only one instance at a time) |
| Time limit | None |
| Battery | Runs on battery; does not stop on battery |

---

### Step 4 — Notify

Configure optional notifications sent when a dump file is captured.

#### Email

| Field | Description |
|-------|-------------|
| **Enable email notifications** | Master toggle. All email fields appear when checked. |
| **SMTP Server** | Hostname or IP of your mail relay (e.g. `smtp.corp.example.com`). |
| **Port** | SMTP port. Common values: `25` (unencrypted relay), `587` (STARTTLS), `465` (implicit SSL). |
| **Use SSL** | Enable TLS/SSL encryption for the SMTP connection. |
| **From** | Sender email address. |
| **To (;-sep)** | One or more recipient addresses, separated by semicolons. |
| **CC (;-sep)** | Optional CC recipients, semicolon-separated. |
| **SMTP User** | Username for SMTP authentication. Leave blank if your relay does not require auth. |
| **Password** | SMTP password. Encrypted with DPAPI (LocalMachine scope) and stored as a Base64 blob in `config.json`. Never written to logs. |
| **Validate SMTP** | Tests TCP connectivity to the SMTP server and port. Does **not** send a message. |
| **Send Test Email** | Sends an actual test email using the configured settings. Use this to confirm end-to-end delivery. |

#### Webhook

| Field | Description |
|-------|-------------|
| **Enable webhook notifications** | Master toggle. |
| **Webhook URL** | HTTP(S) endpoint that receives a POST with a JSON payload when a dump is captured. |

#### Maintenance & Retention *(collapsed by default)*

Click **▶ Maintenance & Retention** to expand.

| Field | Description |
|-------|-------------|
| **Max Log Size (MB)** | Size per log file before rotation. Default: `10`. |
| **Max Log Files** | Number of rotated log files to keep. Default: `5`. |
| **Dump Retention (days)** | Delete `.dmp` files older than this. `0` = disabled (keep forever). |
| **Max Dump GB** | Delete oldest dumps when total dump folder size exceeds this. `0` = disabled. |
| **Stability Timeout (s)** | How long to wait for a `.dmp` file to stop growing before treating it as complete. Default: `30`. Prevents sending notifications for partially-written dumps. |

---

### Step 5 — Review

Read-only summary of all settings and the primary action panel.

#### Action buttons

| Button | What it does |
|--------|-------------|
| **Create Task** / **Update Task** | Saves `config.json` and registers (or updates) the Windows Scheduled Task. This is the primary action. |
| **Run Task Now** | Demand-starts the scheduled task immediately (equivalent to right-click → Run in Task Scheduler). |
| **Stop Task** | Stops the running task. |
| **Remove Task** | Deletes the scheduled task from Task Scheduler. |
| **Save Config Only** | Writes `config.json` without touching Task Scheduler. |
| **Open Dump Folder** | Opens the dump directory in Explorer. |
| **View Logs** | Opens `procdump.log` in Notepad. |
| **Copy ProcDump Cmd** | Copies the full ProcDump command line to the clipboard. |
| **Open Task Scheduler** | Launches `taskschd.msc`. |

#### Status banner & log

The bottom of the page shows a color-coded status banner (green = success, red = error, blue = working) and a scrollable log of operations performed during this session.

---

### Step 6 — About

Displays branding (JCI globe logo), app name, attribution, and the build date stamp.

---

## CLI Reference

The EXE carries an embedded manifest (`requireAdministrator`) so Windows itself prompts for elevation (UAC) whenever it's launched, GUI or CLI alike — there is no in-app re-launch step and no way to opt out from the command line.

```
LogDump.exe [verb] [--config <path>]
```

Every verb below accepts either form: `install` or `--install` (leading dashes are stripped before matching).

| Command | Description |
|---------|-------------|
| *(no arguments)* | Launch the GUI wizard. |
| `monitor --config <path>` | Run the continuous ProcDump monitoring loop (headless). This is what the Scheduled Task executes. |
| `install --config <path>` | Create or update the Scheduled Task from the config file. |
| `uninstall [--config <path>]` | Remove the Scheduled Task. |
| `start [--config <path>]` | Demand-start the task. |
| `stop [--config <path>]` | Stop the running task. |
| `status [--config <path>]` | Print task status as JSON to stdout. |
| `version` | Print the version. |
| `help` | Print usage help. |

`--config` defaults to `config.json` next to the EXE when omitted.

**Exit codes:** `0` = success, `1` = operation failed, `2` = bad arguments.

---

## How It Works

### Monitor Loop

When the scheduled task runs `LogDump.exe monitor`, it enters an infinite loop:

1. **Disk guard** — Check free disk space. If below the configured threshold, skip this cycle and retry after the restart delay.
2. **Launch ProcDump** with `-w` (wait for process) and all configured flags.
3. ProcDump attaches to the target (or waits for it to start) and monitors for the configured trigger conditions.
4. When ProcDump exits (dump captured, process terminated, or trigger fired), **scan** the dump directory for new `.dmp` files.
5. **Stability check** — Poll the dump file size until it stops growing (configurable timeout), ensuring the file is fully written.
6. **Retention cleanup** — Apply age-based and size-based dump retention policies.
7. **Notify** — Send email and/or webhook notifications (with deduplication to avoid duplicate alerts).
8. **Health heartbeat** — Write `health.json` with cycle status, PID, dump count, disk space, and last error.
9. **Sleep** for the configured restart delay.
10. **Repeat** from step 1.

### Why SYSTEM?

The task runs as **SYSTEM** so that:
- It starts automatically at boot, before any user logs in.
- It can attach to services running as SYSTEM, NetworkService, or LocalService.
- It does not depend on any user session.

### Dump Folder Permissions

The dump directory must be writable by SYSTEM. Recommended: `C:\Dumps\<AppName>\`. Avoid `C:\Windows\` or user profile directories.

---

## Known Limitations

- **A panicking notification job aborts the process.** The release profile builds with `panic = "abort"` (part of the size gate), so if a notification job panics it takes the whole `monitor` process down instead of being caught by the notification queue's `catch_unwind`. This is a deliberate trade-off, not an oversight: the Scheduled Task's restart-on-failure policy (1-minute interval, up to 999 retries) is the designed recovery net, so a crash self-heals within a minute rather than silently degrading.
- **Stopping the task can orphan a running `procdump.exe`.** `schtasks /End` (or the wizard's **Stop Task** button) hard-kills the monitor process; if ProcDump is mid-capture at that moment, it is not cleaned up and keeps running as an orphaned child. This matches the original .NET app's behavior — a Windows Job Object tying the child's lifetime to the parent would be the future fix, and was not part of this rewrite's scope.
- **One notification per monitor cycle, even when ProcDump writes several dumps.** Each cycle the monitor notifies for the single newest `.dmp` and advances its cycle marker, so if a preset uses `-n` above 1 (the **High CPU spike** and **Memory threshold** scenarios ship `-n 3`) and ProcDump writes `_1`/`_2`/`_3` in one invocation, only the last triggers an email/webhook, and `health.json`'s `TotalDumpCount` counts cycles-that-captured rather than individual dump files. This matches the original .NET app's behavior. The dumps themselves are all written and retained; only the alerting and the counter collapse per cycle.

---

## Config & Migration

- **Location:** `config.json` next to the EXE.
- **Schema version:** Currently `3`. Stamped on every save.
- **Compatibility, not migration:** field names match the .NET app's schema field-for-field, so a **V3** `config.json` produced by the old .NET build loads directly — no conversion needed. There is **no automatic migration** for pre-V3 configs; any field the parser doesn't recognize is simply ignored, and a missing or unparseable file falls back to defaults.
- **Scenario default:** New configs default to `"Crash capture"`. The UI never auto-selects "Custom" unless an option is hand-edited.

---

## Security Notes

- **SMTP passwords** are encrypted with DPAPI (LocalMachine scope). Any administrator on the machine can decrypt the blob. This is acceptable for a SYSTEM-level monitoring tool.
- **Secrets are never logged.** The logger explicitly omits password fields.
- **Elevation:** the EXE carries an embedded manifest (`requireAdministrator`), so Windows itself prompts for UAC elevation before the process starts — GUI and CLI alike. There is no in-app re-launch step and no way to run un-elevated.
- **Config ACLs:** Secure the folder containing `config.json` to Administrators + SYSTEM.

---

## Building

On Windows (MSVC toolchain required — the GUI, DPAPI, and Task Scheduler code are Windows-only):

```bash
cd rust
cargo build --release
cargo test              # runs the full suite, incl. DPAPI round-trip

# Output
# rust/target/release/LogDump.exe
```

From Linux (LRPC), the same build runs remotely on a Windows VM and the resulting EXE is copied back:

```bash
scripts/vm-build.sh test    # cargo test on the VM
scripts/vm-build.sh build   # cargo build --release on the VM, fetches dist/LogDump.exe
```

`cd rust && cargo test` also runs directly on Linux as a compile/sanity check — it builds and exercises every platform-independent module (config, notify, retention, procdump arg-building, bitness, CLI parsing), skipping only the `#[cfg(windows)]` GUI/DPAPI/Task Scheduler code.

Copy the output EXE plus `procdump64.exe` to your deployment folder. No installer required.

## Crate Dependencies

From `rust/Cargo.toml`:

| Crate | Purpose |
|-------|---------|
| `serde` / `serde_json` | Config (de)serialization, JSON status output |
| `chrono` | Local-time timestamps for logs, emails, and the build-date stamp |
| `base64` | DPAPI blob encoding for `config.json` |
| `lettre` (rustls-tls) | SMTP email sending |
| `ureq` | Webhook HTTP POST |
| `native-windows-gui` / `native-windows-derive` (Windows only) | The six-page wizard GUI |
| `windows` (Windows only) | DPAPI, process enumeration, bitness detection, console attach |
| `winresource` (Windows only, build-dependency) | Embeds the icon and manifest into the EXE |

---

## Credits

The Log Collector half of this tool is a native port of the CCURE LogCollector GUI v2.0, originally written in PowerShell by L3 Production.

---

## Expansion feasibility: all-in-one diagnostics app

LogDump is a Rust application with a native-windows-gui (nwg) wizard today (rewritten from the original .NET 8 WinForms app). The current UX is a six-step wizard for one job: configure ProcDump, install a Scheduled Task, monitor for dumps, and send notifications. The architecture notes below were written against the .NET WinForms implementation and are kept as roadmap context; a Rust hub would follow the same shape but with a Rust service layer and adapters instead of C#.

If the app grows to include the Log Collector Tool and WCF Client Tracing Tool, the recommended direction is to evolve the wizard into a diagnostics hub with separate sections for each workflow.

### Recommended shell

- Replace or surround the wizard with a left navigation or tabbed hub.
- Keep the existing ProcDump wizard as the **Dump Monitoring** section.
- Add shared services for elevation checks, logging, run folders, configuration, output summaries, and support-bundle export.
- Prefer adapter boundaries so legacy PowerShell tools can run out-of-process until their core logic is worth porting.

### WCF Client Trace integration

**Feasibility:** medium effort and a good first integration.

The WCF tracing tool is a compact PowerShell/WinForms script that backs up client config files, injects WCF tracing XML, writes `Client.traces.svclog`, and restores backups. The best long-term approach is to port the XML backup/inject/restore logic into a C# service and expose it as a **WCF Tracing** tab. Keep the script as a fallback launcher during transition.

Primary risks:

- Administrator/UAC handling for files under Program Files.
- Safe XML updates without duplicating existing diagnostics nodes.
- Clear restore workflow so tracing is disabled after log collection.

### Log Collector Tool integration

**Feasibility:** high effort, but practical if introduced as a hosted tool first.

The log collector is a larger PowerShell WinForms application with its own tabs, global UI state, admin detection, VSS-aware collection behavior, and structured output. It is not currently shaped as a reusable library. Start by adding a **Log Collection** section that launches the existing script or compiled EXE and standardizes where output is written. Port individual collectors later only after the shared output and elevation model are stable.

Primary risks:

- Recreating the existing multi-tab PowerShell UI in C# would be costly.
- Some collection paths require admin rights while others are best-effort.
- VSS, event log export, integration-specific collectors, and customer-data handling need careful validation.

### Candidate architecture

| Layer | Responsibility |
| --- | --- |
| Diagnostics shell | Navigation, common status/log pane, admin indicator, tool launch points. |
| Shared services | Elevation, config, run folders, logging, redaction, retention, support bundle export. |
| Tool adapters | ProcDump engine, WCF trace service, hosted log collector runner. |
| Execution layer | In-process C# for native features; out-of-process PowerShell/EXE runner for legacy tools. |
| Output model | One timestamped run folder per workflow with a manifest, transcript/log, and collection summary. |

### Naming workshop

Strongest current name candidates:

| Name | Why it fits |
| --- | --- |
| **DiagHub** | Short, clear, and broad enough for dump, trace, and log workflows. |
| **SupportForge** | Implies a support-engineering toolkit rather than a single-purpose monitor. |
| **TraceDock** | Good fit for collecting, docking, and reviewing traces/logs/dumps. |
| **OpsLens** | Positions the app as a diagnostic lens for production operations. |
| **OneView Diagnostics** | Emphasizes one place for multiple support diagnostics. |

Other viable options: SupportWorks, DeepTrace Toolkit, RescueDesk, SignalForge, TechPulse, IssuePilot, Unified Diagnostics Center, Atlas Support Toolkit.

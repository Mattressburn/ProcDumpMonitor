# LogDump

A single-EXE Windows utility for C•CURE field support. It does two jobs:

- **Watches** a process or service and captures **Sysinternals ProcDump** crash, hang, and resource dumps unattended via a **Scheduled Task**, with retention and optional **email and webhook alerts**.
- **Collects** C•CURE diagnostic log bundles — the evidence a support case needs — into one timestamped folder.

> **A SWH L3 Production** — packaged for C•CURE deployments.

---

## Quick Start

1. Copy the published `LogDump.exe` and `procdump64.exe` into the same folder on the target machine.
2. Run `LogDump.exe` — it will request Administrator privileges automatically.
3. To arm crash capture: on the **ProcDump** page pick a target, then click **Create Task** and **Run Now** in the footer.
4. To produce a support bundle: pick a page under **LOG COLLECTOR** and click its start button. Output lands on the Desktop unless you set a save path.

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

> **Upgrading from a ProcDumpMonitor install:** drop `LogDump.exe` into the
> existing folder beside the current `config.json` — do not create a new folder.
> All settings live next to the EXE, and the saved SMTP password and webhook URL
> are encrypted against this machine with no way to migrate them by hand. An
> already-registered Scheduled Task keeps its old name and keeps working; the app
> reads the name from `config.json` rather than recomputing it.

---

## Requirements

- Windows 10/11 or Windows Server 2016+
- Sysinternals ProcDump (`procdump64.exe` and/or `procdump.exe`)
- **Administrator privileges** (required to create Scheduled Tasks and attach to processes; requested automatically via the EXE's embedded manifest)
- Rust (MSVC toolchain) — only if building from source

---

## The GUI

A freely-clickable sidebar shell — no wizard, no page order. Two groups plus About:

| Group | Page | Purpose |
|-------|------|---------|
| **MONITOR** | **ProcDump** | Everything for crash capture: target, ProcDump options, schedule, notify essentials, live status, and the action footer. |
| **LOG COLLECTOR** | **Data Collection** | The main C•CURE bundle — system info, installed applications and updates, event logs, SWHSystem settings, and this tool's own logs/dumps/task state. |
| | **Install Logs** | C•CURE installer logs and `InstallHistory.xml`. |
| | **System Health** | A point-in-time health snapshot. |
| — | **About** | Branding, attribution, and the build-date stamp. |

Two power-user panels open as separate windows from the ProcDump page: **Advanced…** ("Advanced options" — performance-counter triggers, exception filters, WER registration, log and dump retention) and **SMTP…** ("SMTP settings" — the full mail configuration).

Every collector page has the same shape: tick what to gather, optionally set a save path (blank = Desktop), start it, and use **Open last output** when it finishes. Collection runs on a worker thread, so the window stays responsive.

The start button is named per page — **Start collection** (Data Collection), **Start extraction** (Install Logs), **Collect system health** (System Health). Data Collection also has **Select all** / **Select none**.

---

## ProcDump Page — Field-by-Field Reference

The field tables below are grouped by concern. All of it lives on the single ProcDump page unless a row says otherwise.

---

### Target

Choose the process that ProcDump will monitor.

| Field | Description |
|-------|-------------|
| **Process Name** | The Windows process name **without `.exe`** (e.g. `CrossFireService`, `notepad`). Type it directly, or pick from the service dropdown below and it fills in automatically. |
| **Select Service** | Dropdown listing Windows services on this machine. By default only **Running** services are shown. |
| **Show all services** | Check this to include Stopped/Disabled services in the dropdown. |
| **Refresh Services** | Re-enumerate services (useful if a service started since the app opened). |

**What happens:** When you pick a service, its internal service name (not the display name) is copied into the Process Name field. That name is what ProcDump uses with the `-w` (wait) flag.

> **Tip:** The process name also seeds the auto-generated Scheduled Task name on the Task page.

---

### ProcDump options

Configure *how* ProcDump captures dumps. For most use cases, pick a **Preset** and leave everything else alone.

#### Preset dropdown

One-click shortcuts. Picking one **resets every trigger** and applies that combination; the default is **Crash capture**.

| Preset | ProcDump flags | When to use |
|--------|---------------|-------------|
| **Crash capture** *(default)* | `-ma -e -t` | Process crashes with an unhandled exception or terminates unexpectedly. Standard post-mortem investigation. |
| **Hang capture** | `-ma -h` | Process window stops responding (hung). Diagnose UI freezes and deadlocks. |
| **Crash + hang capture** | `-ma -e -t -h` | Either failure mode. The usual choice for a service that might die or might just wedge, when you don't know in advance which you're chasing. |
| **High CPU spike capture** | `-ma -c 90 -s 10 -n 3` | CPU exceeds 90 % for 10+ seconds. Captures up to 3 dumps. Identify runaway threads. |
| **Memory threshold capture** | `-ma -m 2048 -n 3` | Memory commit exceeds 2048 MB. Captures up to 3 dumps. Investigate memory leaks. |
| **Low impact full dump** | `-a -r -ma` | One-time full dump using process cloning (`-r`). Process suspended for milliseconds, not the full write duration. |
| **Custom** | *(manual)* | Whatever combination you built by hand. The dropdown switches here automatically the moment you change any individual option. |

> **The preset list is not the limit.** A preset picks *one* combination, but the triggers themselves combine freely — tick any mix of **-e**, **-h**, **-t**, the CPU fields, and **MB (-m)** and the dropdown moves to **Custom** while keeping everything you ticked. Use the dropdown to snap back to a preset.

#### Effective command

Read-only preview showing exactly what ProcDump command line will be generated.

#### Target bitness

Auto-detected, and it matters: a 32-bit ProcDump **cannot capture a 64-bit process at all**. LogDump reads the target's PE header on disk — which works even when the target isn't running yet, the normal case when arming with `-w` — and picks `procdump.exe` or `procdump64.exe` accordingly.

For .NET targets it also reads the CLR header, because a managed **AnyCPU** assembly looks 32-bit in its PE `Machine` field but runs 64-bit. Every C•CURE target is .NET, so this is the common path. When the answer genuinely cannot be determined (a 32-bit service hosted in `svchost`, say) the label reads **Unknown** and the 64-bit binary is used.

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

#### Advanced options *(separate window)*

Most users should leave these empty. Click **Advanced…** on the ProcDump page to open the "Advanced options" window.

| Field | ProcDump flag | Description |
|-------|--------------|-------------|
| **Above threshold (-p)** | `-p <counter>` | Performance counter trigger. Dump when counter exceeds a value. |
| **Below threshold (-pl)** | `-pl <counter>` | Performance counter trigger. Dump when counter drops below a value. |
| **Include filter (-f)** | `-f <filter>` | Only capture dumps for exceptions whose name matches this filter (e.g. `OutOfMemory`). |
| **Exclude filter (-fx)** | `-fx <filter>` | Skip dumps for exceptions matching this filter (e.g. `ThreadAbort`). |
| **Register as WER debugger (-wer)** | `-wer` | Register ProcDump as the Windows Error Reporting post-mortem debugger. Only needed when `-e` is not catching crashes because WER intercepts them first. |
| **Avoid-terminate timeout (-at)** | `-at <N>` | Seconds. For rare edge cases where ProcDump blocks process shutdown. Leave at `0`. |

---

### Scheduled task

Configure the Windows Scheduled Task that runs the monitor loop.

| Field | Description |
|-------|-------------|
| **Scheduled Task Name** | Auto-generated as `LogDump <ProcessName>` from the selected target. You can edit it freely; once you do, it stops following the target. |
| **Reset to Auto** | Button that regenerates the task name from the current target. Use this after manually editing if you want the auto-name back. |

The page also shows:

- **Detection badge** — Whether a task with this name already exists. If it does, **Create Task** **updates** it instead of creating a new one.
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

### Notifications

Configure optional notifications sent when a dump file is captured.

#### Email

The ProcDump page carries only the essentials — the **Email** toggle and the recipient list. Everything else is behind **SMTP…**, in the "SMTP settings" window.

| Field | Where | Description |
|-------|-------|-------------|
| **Email** | ProcDump page | Master toggle for email notifications. |
| **To (;-sep)** | ProcDump page | One or more recipient addresses, separated by semicolons. |
| **SMTP server** | SMTP… window | Hostname or IP of your mail relay (e.g. `smtp.corp.example.com`). |
| **Port** | SMTP… window | SMTP port. Common values: `25` (unencrypted relay), `587` (STARTTLS), `465` (implicit SSL). |
| **Use SSL/TLS** | SMTP… window | Enable TLS/SSL encryption for the SMTP connection. |
| **From address** | SMTP… window | Sender email address. |
| **CC (; separated)** | SMTP… window | Optional CC recipients, semicolon-separated. |
| **SMTP username** | SMTP… window | Username for SMTP authentication. Leave blank if your relay does not require auth. |
| **SMTP password** | SMTP… window | Encrypted with DPAPI (LocalMachine scope) and stored as a Base64 blob in `config.json`. Never written to logs. |
| **Validate SMTP** | SMTP… window | Tests TCP connectivity to the SMTP server and port. Does **not** send a message. |
| **Send Test Email** | SMTP… window | Sends an actual test email using the configured settings. Use this to confirm end-to-end delivery. |

#### Webhook

| Field | Description |
|-------|-------------|
| **Webhook** | Master toggle. |
| **Webhook URL** | HTTP(S) endpoint that receives a POST with a JSON payload when a dump is captured. |

#### Maintenance & retention *(in the Advanced options window)*

Under "Logs & retention" in the **Advanced…** window.

| Field | Description |
|-------|-------------|
| **Max Log Size (MB)** | Size per log file before rotation. Default: `10`. |
| **Max Log Files** | Number of rotated log files to keep. Default: `5`. |
| **Dump Retention (days)** | Delete `.dmp` files older than this. `0` = disabled (keep forever). |
| **Max Dump GB** | Delete oldest dumps when total dump folder size exceeds this. `0` = disabled. |
| **Stability Timeout (s)** | How long to wait for a `.dmp` file to stop growing before treating it as complete. Default: `30`. Prevents sending notifications for partially-written dumps. |

---

### Action footer

A row of buttons pinned to the bottom of the window. It is shown only on the ProcDump page — switching to a collector page hides it.

| Button | What it does |
|--------|-------------|
| **Create Task** | Saves `config.json` and registers (or updates) the Windows Scheduled Task. This is the primary action. |
| **Run Now** | Demand-starts the scheduled task immediately (equivalent to right-click → Run in Task Scheduler). |
| **Stop** | Stops the running task. |
| **Remove Task** | Deletes the scheduled task from Task Scheduler. |
| **Save Config** | Writes `config.json` without touching Task Scheduler. |
| **Open Dumps** | Opens the dump directory in Explorer. |
| **View Logs** | Opens `procdump.log` in Notepad. |
| **Copy Args** | Copies the full ProcDump command line to the clipboard. |
| **Task Scheduler** | Launches `taskschd.msc`. |

#### Live status

The ProcDump page polls `schtasks` and `health.json` every 3 seconds and shows the result in a status panel — whether the task exists, its state, and the monitor's last heartbeat. There is no blind "done" message; the panel reflects real system state.

---

## Log Collector

A native port of the three working tabs of the CCURE LogCollector GUI v2.0 (see [Credits](#credits)). No PowerShell script ships with this tool — collection shells only to built-in Windows utilities (`robocopy`, `wevtutil`, `reg.exe`, `systeminfo`, `tar.exe`, and inline `powershell -Command`).

| Page | Gathers |
|------|---------|
| **Data Collection** | System information, installed applications, installed updates, `InstallHistory.xml`, Application + System event logs (last 7 days by default, or a full export), bulk updates, SWHSystem settings, C•CURE log components from the install directory, and LogDump's own logs, dumps, config, and task state. |
| **Install Logs** | C•CURE installer logs and install history. |
| **System Health** | A point-in-time system health snapshot. |

Output goes to a timestamped run folder — `<base>\YYYY-MM-DD\Run_HHMMSS\` — containing the collected trees, a transcript, and a `Collection_Summary.txt`. The layout mirrors the original PowerShell tool so existing JCI support tooling still matches it.

The same engine is available headless via the `collect` verb, and the monitor can fire a bundle automatically when it captures a dump (rate-limited, and skipped when disk is low).

---

## CLI Reference

The EXE carries an embedded manifest (`requireAdministrator`) so Windows itself prompts for elevation (UAC) whenever it's launched, GUI or CLI alike — there is no in-app re-launch step and no way to opt out from the command line.

```
LogDump.exe [verb] [--config <path>]
```

Every verb below accepts either form: `install` or `--install` (leading dashes are stripped before matching).

| Command | Description |
|---------|-------------|
| *(no arguments)* | Launch the GUI. |
| `monitor --config <path>` | Run the continuous ProcDump monitoring loop (headless). This is what the Scheduled Task executes. |
| `install --config <path>` | Create or update the Scheduled Task from the config file. |
| `uninstall [--config <path>]` | Remove the Scheduled Task. |
| `start [--config <path>]` | Demand-start the task. |
| `stop [--config <path>]` | Stop the running task. |
| `status [--config <path>]` | Print task status as JSON to stdout. |
| `collect [--out <dir>] [--workflows <list>]` | Run the log collector headless. `<list>` is any comma-separated mix of `data`, `install`, `health`, `pdm`; the default is all four. Output defaults to the Desktop. |
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
- **Stopping the task can orphan a running `procdump.exe`.** `schtasks /End` (or the footer's **Stop** button) hard-kills the monitor process; if ProcDump is mid-capture at that moment, it is not cleaned up and keeps running as an orphaned child. This matches the original .NET app's behavior — a Windows Job Object tying the child's lifetime to the parent would be the future fix, and was not part of this rewrite's scope.
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
| `native-windows-gui` / `native-windows-derive` (Windows only) | The sidebar-shell GUI and its two dialogs |
| `windows` (Windows only) | DPAPI, process enumeration, bitness detection, console attach |
| `winresource` (Windows only, build-dependency) | Embeds the icon and manifest into the EXE |

---

## Credits

The Log Collector half of this tool is a native port of the CCURE LogCollector GUI v2.0, originally written in PowerShell by L3 Production.

---

## Roadmap: all-in-one diagnostics app

The plan was to grow this from a single-purpose ProcDump wizard into a diagnostics hub with a section per workflow. **That shell now exists** — a left-navigation sidebar with shared elevation, logging, run-folder, and output-summary handling — and the Log Collector is ported into it natively rather than shelling out to the original PowerShell.

One workflow from the original plan is still outstanding.

### WCF Client Trace integration

**Feasibility:** medium effort and a good first integration.

The WCF tracing tool is a compact PowerShell/WinForms script that backs up client config files, injects WCF tracing XML, writes `Client.traces.svclog`, and restores backups. The approach that worked for the Log Collector applies here: port the XML backup/inject/restore logic natively and add it as a third sidebar group rather than shipping a script alongside the EXE.

Primary risks:

- Administrator/UAC handling for files under Program Files.
- Safe XML updates without duplicating existing diagnostics nodes.
- Clear restore workflow so tracing is disabled after log collection.

### Log Collector integration — **done**

Shipped natively. The three working tabs of the original PowerShell tool (Data Collection, Install Logs, System Health) are ported into the sidebar; no script is shipped and nothing runs out-of-process except built-in Windows utilities. The output layout still mirrors the PS1 so existing support tooling matches it.

Not ported, because they were unimplemented stubs in the source script: Controller Logs and Integrations. VSS-aware capture of locked files is also still outstanding.

### Architecture as built

| Layer | Responsibility |
| --- | --- |
| Shell | Sidebar navigation, per-page frames, shared status/log pane, action footer. |
| Shared services | Config, run folders, logging, retention, support-bundle export. |
| Engines | ProcDump monitor loop; native collection engine. |
| Execution layer | In-process Rust; built-in Windows tools (`robocopy`, `wevtutil`, `reg.exe`, `systeminfo`, `tar.exe`) spawned with `CREATE_NO_WINDOW`. |
| Output model | One timestamped run folder per workflow with a transcript and a collection summary. |

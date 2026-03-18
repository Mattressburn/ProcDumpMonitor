# ProcDump Monitor

A Windows utility that configures **Sysinternals ProcDump** as an unattended **Scheduled Task** for crash-dump monitoring, with optional **email and webhook notifications**.

> **A SWH L3 Production** — packaged for C•CURE deployments.

---

## Quick Start

1. Copy the published `ProcDumpMonitor.exe` and `procdump64.exe` into the same folder on the target machine.
2. Run `ProcDumpMonitor.exe` — it will request Administrator privileges automatically.
3. Walk through the six-step wizard described below.
4. On the **Review** page, click **Create Task** then **Run Task Now**.

```
C:\Tools\ProcDumpMonitor\
├── ProcDumpMonitor.exe      ← single-file self-contained EXE
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
- **Administrator privileges** (required to create Scheduled Tasks and attach to processes)
- .NET 8 SDK (only if building from source)

---

## Wizard Pages — Field-by-Field Reference

The GUI is a six-step wizard. Use **Next →** and **← Back** to navigate. The footer also has **Export…** and **Import…** buttons for config portability.

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

#### Filter options

A text box that filters the option groups below by keyword (e.g. type `cpu` to show only CPU options).

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
| **Scheduled Task Name** | Auto-generated as `ProcDump Monitor <ProcessName>` from the target you selected in Step 1. You can edit it freely. |
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
| **Run One-Shot** | End-to-end integration test: creates a **real** scheduled task → creates a **simulated** dump file → sends a **real** email notification → **removes** the task. The task is intentionally ephemeral — it is deleted when the test completes. Use this to validate your email pipeline without needing the target process to actually crash. |
| **Save Config Only** | Writes `config.json` without touching Task Scheduler. |
| **Open Dump Folder** | Opens the dump directory in Explorer. |
| **View Logs** | Opens `procdump.log` in Notepad. |
| **Copy ProcDump Cmd** | Copies the full ProcDump command line to the clipboard. |
| **Open Task Scheduler** | Launches `taskschd.msc`. |
| **Support Diagnostics…** | Packages logs, config, and system info into a ZIP bundle for escalation. |

#### Status banner & log

The bottom of the page shows a color-coded status banner (green = success, red = error, blue = working) and a scrollable log of operations performed during this session.

---

### Step 6 — About

Displays branding (JCI globe logo), app name, attribution, and the build date stamp.

---

## CLI Reference

All CLI commands require an **elevated (Administrator) command prompt** unless noted.

```
ProcDumpMonitor.exe [options]
```

| Command | Description |
|---------|-------------|
| *(no arguments)* | Launch the GUI wizard. |
| `--monitor --config <path>` | Run the continuous ProcDump monitoring loop (headless). This is what the Scheduled Task executes. |
| `--oneshot [--config <path>]` | Single cycle: create task → capture dump → email → remove task → exit. |
| `--oneshot --simulate-dump` | Same as above, but uses a fake ProcDump and fake dump file. |
| `--oneshot --no-email` | Skip email sending during one-shot. |
| `--install --config <path>` | Create or update the Scheduled Task from the config file. |
| `--uninstall [--config <path>]` | Remove the Scheduled Task. |
| `--start [--config <path>]` | Demand-start the task. |
| `--stop [--config <path>]` | Stop the running task. |
| `--status [--config <path>]` | Print task status as JSON to stdout. |
| `--support-diagnostics` | Create a support bundle ZIP. Optionally add `--since` and `--until` with ISO 8601 timestamps. |
| `--export-config <output-path>` | Export config with secrets redacted (safe to share). |
| `--selftest` | Quick smoke test. Exit code `0` = pass. |
| `--version` | Print the assembly version. |
| `--help` | Print usage help. |
| `--no-elevate` | Skip automatic UAC re-launch (for CI / scripting). |

**Exit codes:** `0` = success, `1` = operation failed, `2` = bad arguments.

---

## How It Works

### Monitor Loop

When the scheduled task runs `ProcDumpMonitor.exe --monitor`, it enters an infinite loop:

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

## Config & Migration

- **Location:** `config.json` next to the EXE.
- **Schema version:** Currently `3`. Stamped on every save.
- **Migration:** Older configs (V0/V1/V2) are automatically migrated on load. A `.bak` backup is created before overwriting.
- **Scenario default:** New and migrated configs default to `"Crash capture"`. The UI never auto-selects "Custom".
- **Export/Import:** Export redacts the SMTP password blob and webhook URL. Import forces notifications off until re-enabled.

---

## Security Notes

- **SMTP passwords** are encrypted with DPAPI (`DataProtectionScope.LocalMachine`). Any administrator on the machine can decrypt the blob. This is acceptable for a SYSTEM-level monitoring tool.
- **Secrets are never logged.** The logger explicitly omits password fields.
- **Export redacts** `EncryptedPasswordBlob` and `WebhookUrl` with `<REDACTED>`.
- **Elevation:** The GUI silently re-launches elevated via UAC. CLI commands fail with an error message if not elevated.
- **Config ACLs:** Secure the folder containing `config.json` to Administrators + SYSTEM.

---

## Building

```bash
# Debug build
dotnet build

# Run tests
dotnet test tests/ProcDumpMonitor.Tests/

# Publish single-file self-contained EXE (Release)
dotnet publish -c Release -r win-x64

# Output
# bin\Release\net8.0-windows\win-x64\publish\ProcDumpMonitor.exe
```

Copy the output EXE plus `procdump64.exe` to your deployment folder. No installer required.

## NuGet Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `MailKit` | 4.7.1.1 | SMTP email sending |
| `System.ServiceProcess.ServiceController` | 8.0.1 | Enumerate Windows services |
| `TaskScheduler` | 2.12.2 | Windows Task Scheduler COM wrapper |

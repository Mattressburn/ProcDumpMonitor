# ProcDump Monitor

A Windows desktop utility that configures **Sysinternals ProcDump** monitoring as an unattended **Scheduled Task** and optionally sends **email notifications** when a crash dump is created.

## Features

- WinForms GUI for configuration — no scripts or manual XML editing needed
- Creates a Windows Scheduled Task that runs at startup as SYSTEM with highest privileges
- Monitor mode runs ProcDump in a continuous loop, restarting after each dump
- Optional SMTP email notification when a dump file is captured
- SMTP password stored securely using DPAPI (LocalMachine scope)
- Auto-detects `procdump64.exe` / `procdump.exe` placed beside the EXE
- Portable: config saved as `config.json` next to the EXE

## Requirements

- Windows 10/11 or Windows Server 2016+
- .NET 8 SDK (for building)
- Sysinternals ProcDump (`procdump64.exe` or `procdump.exe`)
- Administrator privileges (required to create Scheduled Tasks)

## Quick Start

1. **Build** the project (see below).
2. Copy the published EXE and `procdump64.exe` into the same folder on the target machine.
3. **Run** `ProcDumpMonitor.exe` as Administrator.
4. Set the **Target Name** (process name without `.exe`).
5. Verify the **ProcDump Path** was auto-detected. If not, browse to it.
6. Set the **Dump Directory** where `.dmp` files should be written.
7. Configure **email settings** if desired.
8. Click **Install / Update** to create the Scheduled Task.
9. Click **Start Now** to begin monitoring immediately.

## Building

### Prerequisites

```
dotnet --version   # Must be 8.0 or later
```

### Build (Debug)

```bash
cd ProcDumpMonitor
dotnet build
```

### Publish (Single-File Self-Contained EXE)

```bash
dotnet publish -c Release -r win-x64 /p:PublishSingleFile=true /p:SelfContained=true
```

The output EXE will be in:

```
bin\Release\net8.0-windows\win-x64\publish\ProcDumpMonitor.exe
```

Copy this EXE plus `procdump64.exe` to your deployment folder.

## Deployment

```
C:\Tools\ProcDumpMonitor\
├── ProcDumpMonitor.exe      ← published single-file EXE
├── procdump64.exe            ← Sysinternals ProcDump
├── config.json               ← auto-created on first save
└── Logs\
    └── procdump.log          ← auto-created by monitor mode
```

No installer is needed. Just unzip and run.

## Architecture

| File | Purpose |
|------|---------|
| `Program.cs` | Entry point; argument parsing; routes to GUI or monitor mode |
| `MainForm.cs` | WinForms UI with all fields, buttons, and validation |
| `Config.cs` | Configuration model, JSON persistence, DPAPI password encryption |
| `TaskSchedulerService.cs` | Create / start / stop / remove Scheduled Tasks |
| `ProcDumpMonitor.cs` | Headless monitor loop — launches ProcDump, detects dumps, emails |
| `EmailNotifier.cs` | SMTP email sending and connectivity validation |
| `Logger.cs` | Thread-safe file logger |

## How It Works

### Scheduled Task

The task is configured to:
- **Trigger**: run at system startup (Boot trigger)
- **Run as**: `SYSTEM` account with highest privileges
- **Action**: `ProcDumpMonitor.exe --monitor --config "path\to\config.json"`
- **Settings**: restart on failure, no idle timeout, no execution time limit

### Monitor Mode (`--monitor`)

When launched with `--monitor`, the EXE runs headless in a continuous loop:

1. Launch ProcDump with `-w` (wait for process) and configured flags
2. ProcDump waits for the target process to start, attaches, and captures a dump on crash/termination
3. After ProcDump exits, scan the dump directory for any new `.dmp` file
4. If found, log the dump and send an email notification (if enabled)
5. Sleep for the configured restart delay
6. Repeat from step 1

### Dump Folder Permissions

The dump directory should be writable by the SYSTEM account. Recommended locations:
- `C:\Dumps\<AppName>\` — create manually or let the tool create it
- A subfolder on a non-system drive

Avoid writing dumps to `C:\Windows\` or user profile directories.

### Why SYSTEM?

The Scheduled Task runs as `SYSTEM` so that:
- It starts automatically at boot, before any user logs in
- It has permission to attach to services running as SYSTEM or NetworkService
- It does not depend on any user session

## Security Notes

- SMTP passwords are encrypted with DPAPI (`DataProtectionScope.LocalMachine`) and stored as a Base64 blob in `config.json`. They are never written to logs.
- The tool does not target any OS-critical processes by default.
- Configuration is stored in the application folder — secure the folder with appropriate ACLs in production.

## NuGet Dependencies

| Package | Purpose |
|---------|---------|
| `Microsoft.Win32.TaskScheduler` | Managed wrapper for Windows Task Scheduler COM API |

All other functionality uses built-in .NET 8 APIs.

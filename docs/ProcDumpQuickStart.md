# ProcDump Quick Start Guide

ProcDump Monitor simplifies collecting crash dumps via Sysinternals ProcDump.
Choose a **Scenario** preset to get started fast, then review the effective
command before creating the scheduled task.

---

## Step-by-step

1. **Target** — pick the process or service to monitor.
2. **ProcDump** — select a Scenario from the dropdown (or choose *Custom*).
3. **Task** — review the scheduled-task name.
4. **Notifications** — optionally enable email or webhook alerts.
5. **Review** — verify the settings, then click **Create Task**.

---

## Scenario Presets

| Scenario | Flags | When to Use |
|---|---|---|
| **Crash capture** *(default)* | `-ma -e -t` | Captures a dump on unhandled exception or process termination — the standard post-mortem setup. |
| **Low impact full dump** | `-a -r -ma` | A full dump equivalent to Task Manager, captured via process cloning for minimal disruption. `-a` prevents dump floods. |
| **Hang capture** | `-ma -h` | Captures a dump when the GUI window stops responding for ≥ 5 seconds. |
| **High CPU spike** | `-ma -c 90 -s 10 -n 3` | Captures up to 3 dumps when CPU exceeds 90 % for 10+ consecutive seconds. |
| **Memory threshold** | `-ma -m 2048 -n 3` | Captures up to 3 dumps when commit charge exceeds 2 GB. |

After selecting a preset you can still adjust individual options; the preset
changes to *Custom* automatically.

---

## Common Options at a Glance

### Dump Type
| Flag | Name | Size | Notes |
|------|------|------|-------|
| `-ma` | Full | Large | All process memory. Best for crash analysis. |
| `-mp` | MiniPlus | Medium | Private memory + stacks. Good balance. |
| `-mm` | Mini | Small | Stacks and handles only. |
| `-mt` | Thread dump | Tiny | Plain-text thread stacks (`.txt`, not `.dmp`). |

### Triggers
| Flag | Description |
|------|-------------|
| `-e` | Dump on unhandled exception |
| `-h` | Dump on hung window (~5 s no response) |
| `-t` | Dump on process termination |

### CPU / Memory
| Flag | Description |
|------|-------------|
| `-c <N>` | CPU % above threshold |
| `-cl <N>` | CPU % below threshold |
| `-s <N>` | Seconds the threshold must be sustained |
| `-n <N>` | Number of dumps before ProcDump exits |
| `-u` | Treat threshold as per-CPU, not total |
| `-m <MB>` | Memory commit above threshold |

### Operational
| Flag | Description |
|------|-------------|
| `-r` | Clone / reflect — minimal process suspension |
| `-a` | Avoid outage — exit on rapid triggers |
| `-o` | Overwrite existing dump files |
| `-w` | Wait for the process to launch |
| `-accepteula` | Accept Sysinternals EULA (always on) |

---

## Bitness Detection

ProcDump Monitor automatically detects whether the target process is 32-bit
or 64-bit and selects the matching binary:

| Target | Binary |
|--------|--------|
| 32-bit (WOW64) | `procdump.exe` |
| 64-bit (native) | `procdump64.exe` |
| Not running / unknown | `procdump64.exe` (default on 64-bit OS) |

If the preferred binary is not found beside the application, ProcDump Monitor
falls back to the other binary and shows a warning.

Detection uses `IsWow64Process2` (Windows 10 1709+) with an automatic
fallback to `IsWow64Process` on older versions.

---

## Viewing the Effective Command

The **Effective command** textbox on the ProcDump page updates live as you
change options. The same command is shown in the Review page and can be
copied to clipboard with the **Copy ProcDump Cmd** button.

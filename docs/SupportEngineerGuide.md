# How to Use ProcDump Monitor (Support Engineer Guide)

## What is ProcDump Monitor?

ProcDump Monitor is a wrapper around Sysinternals **ProcDump** that makes it
easy to set up unattended crash-dump collection on production Windows machines.
It creates a Windows Scheduled Task that automatically attaches ProcDump to a
target process, collects dump files when a trigger fires, and optionally sends
email or webhook notifications.

You do **not** need to know ProcDump command-line flags to use it.

---

## The 90 % Workflow (3 Steps)

Most support cases only need three clicks:

| Step | Page | What to Do |
|------|------|------------|
| **1. Choose target** | Target (Page 1) | Type the process name (e.g., `CrossFireService`) or browse for the EXE. |
| **2. Leave the default scenario** | ProcDump (Page 2) | The **Crash capture** scenario is already selected. It captures a full dump on unhandled exceptions and process termination. **You do not need to change anything.** |
| **3. Click Next → Create** | Review (Page 5) | Verify the summary, then click **Create Task**. The Scheduled Task is created and ProcDump begins monitoring. |

That's it. The dump files will appear in the configured dump directory whenever
the target process crashes.

---

## Scenario Reference

| Scenario | Flags | Plain-English Description |
|----------|-------|--------------------------|
| **Crash capture** *(default)* | `-ma -e -t` | Captures a full memory dump when the process throws an unhandled exception or terminates unexpectedly. The standard choice for post-mortem crash investigation. |
| **Low impact full dump** | `-a -r -ma` | A full memory dump equivalent to what Task Manager produces, but captured via process cloning so the running process is suspended for only milliseconds. See [details below](#what-low-impact-full-dump-really-means). |
| **Hang capture** | `-ma -h` | Captures a full dump when the process window stops responding for ≥ 5 seconds. Useful for UI freezes and deadlocks. |
| **High CPU spike capture** | `-ma -c 90 -s 10 -n 3` | Captures up to 3 full dumps when CPU usage exceeds 90 % for at least 10 consecutive seconds. Helps identify runaway threads. |
| **Memory threshold capture** | `-ma -m 2048 -n 3` | Captures up to 3 full dumps when process memory commit exceeds 2 GB. Useful for memory-leak investigations. |
| **Custom** | *(user-defined)* | Full manual control over all ProcDump flags. Only select this if you know exactly what you need. |

After selecting a preset you can still adjust individual options; the dropdown
automatically switches to **Custom** when you do.

---

## Do I Need Page 2? (Usually No.)

Page 2 (ProcDump) shows every ProcDump option the tool supports. **The default
Crash capture scenario already configures all the options you need.** You only
need to visit Page 2 if:

- You want a **different** scenario (e.g., Hang capture, High CPU spike).
- You need to change the **dump directory** or **ProcDump path**.
- A senior engineer or support escalation has asked you to set specific flags.

The **Advanced Options** section at the bottom of Page 2 (Performance Counters,
Exception Filtering, WER Integration, Avoid-terminate timeout) is almost never
needed. Each section includes a "When to use" note — if the note does not
describe your situation, leave it blank.

---

## What "Low Impact Full Dump" Really Means

### How it differs from a normal full dump

A normal full dump (like clicking **Create dump file** in Task Manager)
suspends the target process for the entire time it takes to write the dump to
disk. For a process using several gigabytes of memory this can mean 10–30
seconds of downtime.

**Low impact full dump** uses ProcDump's `-r` (clone / reflect) flag. Instead
of writing directly from the live process, ProcDump asks the OS to create a
snapshot (clone) of the process. The live process is suspended for only the
few milliseconds it takes to create the snapshot, then immediately resumes.
The dump is then written from the snapshot in the background.

### How it compares to Task Manager

| | Task Manager | Low Impact Full Dump |
|---|---|---|
| **Dump content** | Full process memory | Full process memory (identical) |
| **Process suspension** | Entire write duration (seconds) | A few milliseconds |
| **Trigger** | Manual right-click | Automatic (Scheduled Task) |
| **Flood protection** | None | `-a` flag prevents dump floods |

### Why cloning reduces disruption

The OS-level snapshot (`PssCaptureSnapshot`) duplicates the process's memory
pages using copy-on-write semantics. No physical memory is copied at snapshot
time — pages are duplicated lazily only when the live process writes to them
afterward. This makes the snapshot nearly instant regardless of process size.

---

## Notifications (Optional)

On Page 4 you can optionally enable:

- **Email** — sends a notification when a new dump file is detected.
- **Webhook** — posts a JSON payload to a URL (e.g., Microsoft Teams, Slack).

Both are entirely optional and do not affect dump collection.

---

## Quick Checklist

- [ ] Target process name entered on Page 1
- [ ] Scenario = **Crash capture** (default, no change needed)
- [ ] ProcDump path detected automatically (green indicator on Page 2)
- [ ] Dump directory is set to a folder with adequate free space
- [ ] Click **Create Task** on Page 5
- [ ] Verify the Scheduled Task is running (Page 5 shows status)

---

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| "ProcDump executable not found" | ProcDump EXEs not beside the application | Copy `procdump.exe` and `procdump64.exe` into the application folder, or browse to their location on Page 2. |
| No dumps appear after a crash | Target name mismatch | Verify the process name on Page 1 matches exactly (without `.exe`). |
| Task shows "not running" | Elevation required | The Scheduled Task runs as SYSTEM. Click the elevation banner at the top of the wizard to relaunch as Administrator. |
| Dumps are very large | Expected for full dumps | A full dump equals the process's committed memory. Use **MiniPlus** dump type on Page 2 if size is a concern. |

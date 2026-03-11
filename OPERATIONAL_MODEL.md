# ProcDump Monitor -- Operational Model

Operational invariants, failure modes, recovery guarantees, and support playbook.
Written for an L3 support engineer working an incident at 2 AM.

All references are to the codebase as it exists on the `main` branch.

---

## 1. Core Invariants

These assumptions must hold for the system to function correctly. If any are violated, the behavior described in Section 2 or Section 3 applies.

### 1.1 File system

| Invariant | Why | Violated by |
|---|---|---|
| The directory containing `ProcDumpMonitor.exe` is writable by the running user (SYSTEM for the scheduled task). | `config.json`, `health.json`, and `Logs/` are created next to the EXE. See `AppPaths.cs`. | Placing the EXE in a read-only share or a folder with restrictive ACLs. |
| `config.json` exists next to the EXE before `--monitor` is invoked (or a `--config` path is supplied). | `Config.Load()` returns a blank config if the file is missing. The monitor will run with empty `TargetName`, `ProcDumpPath`, and `DumpDirectory`, causing ProcDump to fail immediately every cycle. | Deleting config after task creation; deploying without running the GUI first. |
| The dump directory (`Config.DumpDirectory`) exists or is creatable. | `ProcDumpMonitorLoop.Run` calls `Directory.CreateDirectory()` once at startup. If that fails, the method returns immediately. | Permission denied; invalid path; drive not mounted. |
| `procdump.exe` or `procdump64.exe` exists at `Config.ProcDumpPath`. | Passed directly to `ProcessStartInfo.FileName`. A missing binary causes a `Win32Exception` every cycle. | Moving or deleting ProcDump after configuration. |

### 1.2 Scheduled task

| Invariant | Why |
|---|---|
| The task runs as `SYSTEM` with `RunLevel.Highest`. | Set in `TaskSchedulerService.cs` lines 68-70. Required for access to dump directory, logs, and DPAPI `LocalMachine` decryption. |
| The task has a `BootTrigger` and `AllowDemandStart = true`. | The monitor starts automatically on reboot. Operators can also demand-start it from the GUI, CLI (`--start`), or Task Scheduler. |
| `RestartInterval = 1 minute`, `RestartCount = 999`. | If the monitor process crashes, the Task Scheduler restarts it. This is the **only** external watchdog. |
| `MultipleInstances = IgnoreNew`. | Prevents two monitors from running simultaneously for the same task name. |
| `ExecutionTimeLimit = Zero`. | The monitor runs indefinitely. A non-zero limit would kill the monitor after the timeout. |

### 1.3 ProcDump behavior

| Invariant | Why |
|---|---|
| ProcDump is invoked with `-w` (wait mode). | `Config.BuildProcDumpArgs()` always appends `-w {TargetName}`. ProcDump will block until the target process appears, then attach. |
| ProcDump exits when it captures `MaxDumps` dumps or the target process exits. | The monitor loop detects ProcDump exit, looks for new `.dmp` files, and restarts a new cycle. |
| ProcDump writes `.dmp` files to the `WorkingDirectory`. | `ProcessStartInfo.WorkingDirectory` is set to `cfg.DumpDirectory`. The final argument in the args string is `"{DumpDirectory}"`. |
| ProcDump exits with code 0 on success. | The exit code is logged and stored in `HealthStatus.LastProcDumpExitCode`. Non-zero does not change the restart behavior. |

### 1.4 Notifications

| Invariant | Why |
|---|---|
| The SMTP server is reachable from the machine running the monitor. | `EmailNotifier.Send` opens a synchronous TCP connection with a 30-second timeout. No proxy support. |
| DPAPI `LocalMachine` scope can decrypt the stored password blob. | The blob was encrypted on the same machine. Moving `config.json` to another machine makes the password unrecoverable. |
| The webhook URL accepts POST with `Content-Type: application/json`. | `WebhookNotifier` posts a `WebhookPayload` JSON body. A non-2xx response is logged but not retried. |

### 1.5 Path resolution

| Invariant | Why |
|---|---|
| `Environment.ProcessPath` returns the real on-disk EXE path. | `AppPaths.InstallDir` derives all file paths from this. In single-file publish, `AppContext.BaseDirectory` may point to a temp extraction directory; `ProcessPath` does not. |
| The EXE is not moved after the process starts. | `AppPaths.InstallDir` is computed once and cached in a static field (`_installDir`). |

---

## 2. Expected Failure Modes

These failures are anticipated. The system handles them without operator intervention or with self-recovery.

### 2.1 Target process not running

**Trigger:** The target process named in `Config.TargetName` is not running when ProcDump starts.

**Symptoms:**
- ProcDump sits in wait mode (`-w` flag). No log output beyond the initial "Waiting for process" line from ProcDump stdout.
- `health.json`: `ProcDumpPid` is non-zero, `LastCycleUtc` updates every 30 seconds (heartbeat), `LastDumpFileName` is unchanged.
- No dumps created. No notifications sent.

**Recovery:** Automatic. ProcDump will attach when the target process starts.

### 2.2 Dump directory does not exist

**Trigger:** `Config.DumpDirectory` path is invalid or the drive is not mounted at startup.

**Symptoms:**
- Log entry: `"Cannot create dump directory: {message}"`.
- `ProcDumpMonitorLoop.Run` returns immediately. The process exits.
- The scheduled task records a non-zero exit code and restarts the monitor in 1 minute.

**Recovery:** Automatic once the path becomes valid (e.g., drive mounts). The task restarts every minute up to 999 times.

### 2.3 ProcDump binary not found

**Trigger:** `Config.ProcDumpPath` does not point to an existing file.

**Symptoms:**
- `Process.Start()` throws `Win32Exception`.
- Caught by the `try/catch` in the cycle loop (line 106-110 of `ProcDumpMonitor.cs`).
- `health.json`: `LastError` contains the exception message. Cycle restarts after `RestartDelaySeconds`.
- Log entry: `"Cycle error: {message}"`.

**Recovery:** Automatic retry every `RestartDelaySeconds`. See Section 3.1 for the log-flooding risk.

### 2.4 Disk space below threshold

**Trigger:** Free space on the dump volume is below `Config.MinFreeDiskMB`.

**Symptoms:**
- Log entry: `"Skipping cycle -- only {N} MB free..."`.
- `health.json`: `DiskSpaceLow = true`, `FreeDiskMB` shows the current value.
- A low-disk notification (email/webhook) is sent once per hour.
- ProcDump is **not** launched. The cycle sleeps and retries.

**Recovery:** Automatic. When free space rises above the threshold, the next cycle proceeds normally.

### 2.5 Dump file still locked after stability timeout

**Trigger:** ProcDump (or another process) holds a lock on the `.dmp` file for longer than `DumpStabilityTimeoutSeconds`.

**Symptoms:**
- Log entry: `"Dump file still locked -- skipping notification."`.
- `health.json`: `LastError` = `"Dump file still locked after timeout -- notification suppressed."`.
- The dump **exists on disk** but no email/webhook is sent for it.

**Recovery:** On the next cycle, if a new dump is created, it will be detected and notified. The locked dump's notification is permanently lost.

### 2.6 SMTP server temporarily unreachable

**Trigger:** Network interruption, server restart, DNS failure.

**Symptoms:**
- Log entry: `"NotifyQ: EmailNotifierAdapter: dump notification failed: {message}"`.
- `health.json`: No change (notification failures do not set `LastError`).
- The notification is **not retried**. It is lost.

**Recovery:** The next dump event will trigger a new notification. The missed notification is not recoverable.

### 2.7 Webhook endpoint returns non-2xx

**Trigger:** Endpoint is down, misconfigured, or rejecting requests.

**Symptoms:**
- Log entry: `"Webhook returned {statusCode}: {reason}"` or `"Webhook request timed out (15s)."`.
- No retry. Notification is lost.

**Recovery:** Same as 2.6.

### 2.8 Config file is corrupt or missing on load

**Trigger:** `config.json` is deleted, truncated, or contains invalid JSON.

**Symptoms:**
- `Config.Load()` catches the exception and returns `new Config { ConfigVersion = CurrentVersion }`.
- All fields are at their defaults: empty `TargetName`, empty `ProcDumpPath`, etc.
- The monitor starts but ProcDump fails immediately every cycle (see 2.3).

**Recovery:** Restore `config.json` from `config.json.bak` (created by `ConfigMigrator.BackupIfNeeded` on every save) and restart the task.

### 2.9 Monitor process crashes

**Trigger:** Unhandled exception outside the cycle try/catch, out-of-memory, access violation.

**Symptoms:**
- `health.json`: `LastCycleUtc` stops advancing. `NextRetryUtc` is in the past.
- The scheduled task detects the non-zero exit and restarts the process after `RestartInterval` (1 minute).

**Recovery:** Automatic via Task Scheduler restart. State is preserved in `health.json` (`TotalDumpCount`, `LastNotifiedDumpFile`).

---

## 3. Dangerous Failure Modes

These failures can cause operational harm if not detected and addressed.

### 3.1 Log flooding from rapid ProcDump restarts

**Trigger:** ProcDump exits immediately every cycle. Common causes:
- `ProcDumpPath` points to a non-existent or wrong binary.
- ProcDump arguments are invalid (e.g., conflicting flags).
- Permission denied launching ProcDump.

**Why it is dangerous:** There is **no backoff**. The loop restarts every `RestartDelaySeconds` (default 5 seconds). Each cycle writes multiple log lines:
- `"-- Cycle start --"`
- `"Executing: ..."`
- `"Cycle error: ..."`
- `"No new dump file detected in this cycle."`
- `"Sleeping Ns before restart..."`

At default settings (5-second delay), this produces roughly 60-70 log lines per minute. Log rotation (default 10 MB, 5 files = 50 MB total) bounds the disk impact, but the logs become useless because they contain only the same repeating error.

**How to detect:**
- `health.json`: `LastError` contains the same message every cycle. `LastProcDumpExitCode` is non-zero or -1. `TotalDumpCount` is not advancing.
- Log file: Repeating identical error blocks.

**Mitigation:** Fix the `ProcDumpPath` or arguments in `config.json` and restart the task.

### 3.2 Disk exhaustion from dump files

**Trigger:** ProcDump creates large full-memory dumps (`-ma`) repeatedly, and:
- `MinFreeDiskMB` is set to 0 (disk guard disabled), **or**
- `DumpRetentionDays` and `DumpRetentionMaxGB` are both 0 (retention disabled).

**Why it is dangerous:** A full-memory dump of a process using 8 GB of RAM produces an 8 GB `.dmp` file. If the target process crashes frequently and no retention policy is active, the dump directory grows without bound. When the disk fills:
- The OS may become unstable.
- ProcDump itself may fail to write.
- Other services on the same volume may fail.

The disk guard only checks **before** launching ProcDump. If ProcDump is already running and the disk fills during dump creation, the guard cannot intervene.

**How to detect:**
- `health.json`: `DiskSpaceLow = true` (if the guard is enabled). `FreeDiskMB` shows low values.
- OS-level disk space monitoring.

**Mitigation:** Set `MinFreeDiskMB` to a non-zero value (default is 5120 = 5 GB). Set `DumpRetentionDays` and/or `DumpRetentionMaxGB` to non-zero values.

### 3.3 Silent monitoring failure (monitor running, ProcDump not attaching)

**Trigger:** ProcDump is in wait mode (`-w`) because the target process name does not match any running process. The process exists under a different name, or the name was misspelled in config.

**Why it is dangerous:** The monitor appears healthy:
- `health.json`: `ProcDumpPid` is non-zero, `LastCycleUtc` advances every 30 seconds, `LastError` is empty.
- Logs show ProcDump started successfully.
- **No dumps are created. No notifications are sent.** There is no alert that the target was never found.

An operator checking `health.json` or `--status` sees a "Running" task with no errors and may assume everything is working.

**How to detect:**
- `health.json`: `TotalDumpCount` stays at 0 or does not change over a long period. `LastDumpFileName` is empty or stale.
- Log: ProcDump stdout shows `"Waiting for process..."` continuously with no attach message.

**Mitigation:** Verify the exact process name. Note that ProcDump matches by image name (e.g., `notepad` not `notepad.exe` in some versions; behavior varies by ProcDump version). Check the ProcDump output in the log.

### 3.4 Silent notification loss

**Trigger:** Any of these:
1. SMTP server rejects the email (auth failure, relay denied, recipient rejected).
2. Webhook endpoint is unreachable.
3. Notification queue is full (64 items, dropped with a log warning).
4. `NotificationQueue` is disposed before the worker thread drains.

**Why it is dangerous:** There is **no retry** for failed notifications. There is no dead-letter queue. There is no health.json field that records notification failures. The only evidence is a log line with category `"NotifyQ"`.

An operator who relies solely on email or webhook alerts to know about dumps will not learn of them.

**How to detect:**
- Log: Search for `"NotifyQ"` entries containing `"failed"` or `"dropping"`.
- Compare `health.json` `TotalDumpCount` against the number of emails received.
- `health.json` `LastNotifiedDumpFile` and `LastNotifiedUtc` show the last **enqueued** notification, not whether delivery succeeded.

**Mitigation:** Periodically check the dump directory manually or via a separate monitoring system. Do not rely exclusively on notifications.

### 3.5 DPAPI password becomes undecryptable

**Trigger:** `config.json` is copied to a different machine. The `EncryptedPasswordBlob` was encrypted with `DataProtectionScope.LocalMachine` on the original machine.

**Why it is dangerous:** `Config.GetPassword()` returns an empty string on decryption failure (line 121 of `Config.cs`). The monitor proceeds with an empty password. MailKit's `Authenticate()` call is skipped when `SmtpUsername` is non-empty but the password is empty -- actually, it calls `Authenticate(new NetworkCredential(username, ""))`, which will likely fail with an auth error. The email notification is lost silently (caught and logged in `NotificationQueue`).

**How to detect:**
- Log: `"NotifyQ: EmailNotifierAdapter: dump notification failed: ..."` with an authentication error.
- `--export-config` shows `<REDACTED>` for the blob, confirming a blob exists but not whether it is valid.

**Mitigation:** After copying config to a new machine, re-enter the SMTP password via the GUI or clear `EncryptedPasswordBlob` and `SmtpUsername` and use anonymous relay.

### 3.6 health.json write failure

**Trigger:** The install directory becomes read-only, or the disk is full, or the file is locked by another process.

**Why it is dangerous:** `HealthWriter.Write` catches all exceptions and logs them (line 55-59 of `HealthWriter.cs`). The monitor continues running, but `health.json` becomes stale. Any external monitoring that depends on polling `health.json` will think the monitor is dead.

**How to detect:**
- Log: `"Health: Failed to write health.json: {message}"`.
- `health.json` on disk has an old `LastCycleUtc`.

**Mitigation:** Fix the underlying file system issue. The monitor itself is unaffected.

---

## 4. Recovery Guarantees

### 4.1 What the scheduled task guarantees

The Windows Task Scheduler provides these properties (set in `TaskSchedulerService.cs`):

| Property | Value | Effect |
|---|---|---|
| `BootTrigger` | Enabled | Monitor starts on every system boot. |
| `StartWhenAvailable` | `true` | If the trigger was missed (e.g., system was off), the task starts as soon as possible. |
| `RestartInterval` | 1 minute | After a crash, the task restarts after 1 minute. |
| `RestartCount` | 999 | Up to 999 consecutive restart attempts. |
| `AllowDemandStart` | `true` | Operators can start the task manually at any time. |
| `StopIfGoingOnBatteries` | `false` | Continues on battery power (relevant for laptops). |
| `DisallowStartIfOnBatteries` | `false` | Starts even on battery. |
| `ExecutionTimeLimit` | `Zero` | No timeout. Runs indefinitely. |

**The task does NOT guarantee:**
- That the monitor process is healthy (only that it is running).
- That ProcDump is attached to the target.
- That notifications are being delivered.

### 4.2 What the monitor loop guarantees

- ProcDump **will be restarted** after every exit, with a fixed delay of `RestartDelaySeconds`.
- Dump detection checks for `.dmp` files created **after the cycle start time** (`f.LastWriteTimeUtc >= cycleStart`).
- Dump stability is verified (size stable + exclusive lock) before notification is enqueued.
- Duplicate notifications are suppressed per dump file name via `HealthStatus.LastNotifiedDumpFile`.
- `health.json` is updated atomically (temp + rename) at the end of every cycle and every 30 seconds during ProcDump wait.
- `TotalDumpCount` persists across monitor restarts (loaded from `health.json` on startup).
- Log rotation bounds total log size to approximately `MaxLogSizeMB * (MaxLogFiles + 1)`.

### 4.3 What the monitor loop does NOT guarantee

- **Notification delivery.** Enqueuing is non-blocking. If the email or webhook call fails, the notification is lost. There is no retry. The `LastNotifiedDumpFile` field records that the notification was **enqueued**, not that it was **delivered**.
- **Dump detection for externally created files.** Only `.dmp` files with `LastWriteTimeUtc >= cycleStart` are detected. Dumps created by a different ProcDump instance or copied into the directory are ignored.
- **Backoff on repeated failures.** The restart delay is fixed. The loop does not distinguish between a ProcDump that ran for 6 hours and one that exited in 100 ms.
- **Target process validation.** The monitor does not verify that the process name in `TargetName` corresponds to any real process. ProcDump silently waits.
- **Config hot-reload.** Config is read once at startup. Changes to `config.json` require restarting the monitor (stop and start the task).
- **Graceful queue drain on crash.** If the monitor process crashes, the `NotificationQueue` background thread is killed. Any in-flight notification is lost.

---

## 5. Operator Playbook

### 5.1 ProcDump never creates dumps

**Scenario:** The monitor has been running for hours/days, but the dump directory is empty.

**Check these, in order:**

1. **Is the task running?**
   ```
   ProcDumpMonitor.exe --status
   ```
   Look at the `State` field. If `"Running"`, the process is alive.

2. **Is ProcDump actually running?**
   Open `health.json`. Check `ProcDumpPid`. If non-zero, ProcDump is alive. If zero, the monitor is between cycles.

3. **Is ProcDump waiting for the target?**
   Open `Logs\procdump.log`. Search for `[ProcDump]` lines. If you see `"Waiting for process..."` with no subsequent attach message, the target process name does not match.

   **Fix:** Verify `Config.TargetName` matches the exact process image name. Open Task Manager, find the process, and note the name in the "Name" column (without `.exe`). Update `config.json` and restart the task.

4. **Is ProcDump erroring out?**
   In the log, search for `[ProcDump-ERR]` lines. Check `health.json` `LastProcDumpExitCode`. Non-zero exit codes indicate ProcDump-level failures (invalid args, access denied, etc.).

5. **Are the triggers correct?**
   Check that at least one of `-e` (exception), `-t` (terminate), `-c` (CPU), `-m` (memory), or `-h` (hang) is active in the config. If no trigger fires, no dump is created even though ProcDump is attached.

   Run this to see the full argument string:
   ```
   ProcDumpMonitor.exe --status
   ```
   Then check `config.json` fields: `DumpOnException`, `DumpOnTerminate`, `CpuThreshold`, `MemoryCommitMB`, `HangWindowSeconds`.

6. **Is disk space sufficient?**
   Check `health.json` `DiskSpaceLow` and `FreeDiskMB`. If `DiskSpaceLow = true`, the cycle is being skipped.

### 5.2 Dumps created but no emails

**Scenario:** `.dmp` files appear in the dump directory, but no email notifications arrive.

**Check these:**

1. **Is email enabled?**
   Open `config.json`. Check `EmailEnabled`. Must be `true`.

2. **Were notifications enqueued?**
   Open `health.json`. Check `LastNotifiedDumpFile` and `LastNotifiedUtc`. If these match the latest dump file name and a recent timestamp, the notification was enqueued.

3. **Did the notification fail?**
   Search `Logs\procdump.log` for `[NotifyQ]` entries:
   - `"EmailNotifierAdapter: dump notification sent."` = success.
   - `"EmailNotifierAdapter: dump notification failed: {error}"` = failure. The error message will indicate the cause (auth failure, connection refused, timeout, etc.).

4. **Is SMTP reachable from this machine?**
   From the machine running the monitor:
   ```powershell
   Test-NetConnection -ComputerName <smtp-server> -Port <smtp-port>
   ```
   Or use the GUI's "Validate SMTP" button.

5. **Is the password valid?**
   If `SmtpUsername` is set and `EncryptedPasswordBlob` is non-empty, the password was encrypted on this machine with DPAPI. If config was copied from another machine, the password is undecryptable. Re-enter it via the GUI.

6. **Was the dump stability check skipped?**
   Search the log for `"Dump file still locked"`. If the dump was locked longer than `DumpStabilityTimeoutSeconds`, the notification was suppressed entirely.

7. **Was the notification deduplicated?**
   Search the log for `"Dump already notified -- skipping duplicate notification."`. This happens if the same `.dmp` file name appears in consecutive cycles.

8. **Was the queue full?**
   Search the log for `"Notification queue full; dropping item."`. This only happens under extreme load (64 pending items).

### 5.3 Task exists but monitor is not running

**Scenario:** `--status` shows the task exists with `State = "Ready"` but the monitor process is not running.

**Check these:**

1. **Was the task started?**
   `State = "Ready"` means the task is registered but not running. It will start on next boot (BootTrigger) or on demand.

   To start it now:
   ```
   ProcDumpMonitor.exe --start
   ```

2. **Did the task fail to start?**
   Check `LastRunResult` in the `--status` output. Common HRESULT values:
   - `0x0` = success (ran and exited).
   - `0x1` = operation failed (check logs).
   - `0x41301` = task is currently running.
   - `0x41325` = task queued.

3. **Has the restart count been exhausted?**
   The task has `RestartCount = 999`. After 999 consecutive failures within the restart interval, the task stops restarting. This is rare but possible.

   **Fix:** Manually start the task, or delete and re-create it via the GUI.

4. **Was the task disabled?**
   Check in Task Scheduler (`taskschd.msc`) whether the task is enabled.

### 5.4 health.json is stale

**Scenario:** `health.json` `LastCycleUtc` is more than a few minutes old.

**Interpretation depends on `ProcDumpPid`:**

| `ProcDumpPid` | `LastCycleUtc` age | Meaning |
|---|---|---|
| Non-zero | < 2 minutes | Normal. ProcDump is running. Heartbeat updates every 30s. |
| Non-zero | > 2 minutes | Possible: (a) health.json write failed (check log for `"Failed to write health.json"`), or (b) the monitor process is hung. |
| 0 | < `RestartDelaySeconds` + 30s | Normal. Between cycles. |
| 0 | >> `RestartDelaySeconds` | The monitor process is likely dead. Check Task Scheduler. |

**Steps:**

1. Check if the process is running:
   ```powershell
   Get-Process ProcDumpMonitor -ErrorAction SilentlyContinue
   ```

2. If not running, check Task Scheduler state:
   ```
   ProcDumpMonitor.exe --status
   ```

3. If running but `health.json` is not updating, check the log for file system errors:
   ```powershell
   Select-String "Failed to write health.json" Logs\procdump.log
   ```

4. If the process appears hung (running, `ProcDumpPid` non-zero, no heartbeat updates):
   - Kill the monitor process.
   - The scheduled task will restart it in 1 minute.
   - Check logs for the cycle that was running when it hung.

### 5.5 Quick-reference: key artifacts and their locations

All paths are relative to the directory containing `ProcDumpMonitor.exe`.

| Artifact | Path | Format | Key fields |
|---|---|---|---|
| Configuration | `config.json` | JSON | `TargetName`, `ProcDumpPath`, `DumpDirectory`, `EmailEnabled` |
| Config backup | `config.json.bak` | JSON | Previous version (created on every save) |
| Health status | `health.json` | JSON | `LastCycleUtc`, `LastError`, `TotalDumpCount`, `DiskSpaceLow`, `ProcDumpPid` |
| Current log | `Logs\procdump.log` | Text | Timestamped, categorized lines |
| Rotated logs | `Logs\procdump.log.1` through `.{MaxLogFiles}` | Text | Older entries; `.1` is most recent |
| Dump files | `{DumpDirectory}\*.dmp` | Binary | Created by ProcDump |

### 5.6 Quick-reference: CLI commands

All commands run from the directory containing `ProcDumpMonitor.exe`.

| Goal | Command |
|---|---|
| Check task state | `ProcDumpMonitor.exe --status` |
| Start the task now | `ProcDumpMonitor.exe --start` |
| Stop the task | `ProcDumpMonitor.exe --stop` |
| Remove the task | `ProcDumpMonitor.exe --uninstall` |
| Re-create the task from config | `ProcDumpMonitor.exe --install --config config.json` |
| Export config (redacted) for sharing | `ProcDumpMonitor.exe --export-config config_export.json` |
| Run self-test (all fakes) | `ProcDumpMonitor.exe --selftest` |
| Print version | `ProcDumpMonitor.exe --version` |

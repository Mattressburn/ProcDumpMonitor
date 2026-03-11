using System.Diagnostics;

namespace ProcDumpMonitor;

/// <summary>
/// Runs in --monitor mode: infinite loop that launches ProcDump, waits for it,
/// detects new .dmp files, checks stability, notifies, and restarts.
/// Integrates: disk-space guard, dump stability check, health heartbeat,
/// retention cleanup, and the notifier pipeline (email + webhook).
/// </summary>
public static class ProcDumpMonitorLoop
{
    private static volatile bool _stopping;

    // Notifier pipeline
    private static readonly INotifier[] Notifiers = { new EmailNotifierAdapter(), new WebhookNotifier() };

    // Persistent state across cycles
    private static HealthStatus _health = new();
    private static DateTime _lastLowDiskNotifyUtc = DateTime.MinValue;

    public static void Run(Config cfg)
    {
        _stopping = false;

        // Configure logger rotation from config
        Logger.MaxLogSizeMB = cfg.MaxLogSizeMB;
        Logger.MaxLogFiles = cfg.MaxLogFiles;

        // Resume persistent state (e.g. TotalDumpCount survives restart)
        _health = HealthWriter.Load();
        _health.MonitorPid = Environment.ProcessId;
        _health.Version = typeof(ProcDumpMonitorLoop).Assembly.GetName().Version?.ToString() ?? "1.0.0";

        Logger.Log("Monitor", "ProcDump Monitor started.");
        Logger.Log("Monitor", $"Target: {cfg.TargetName}");

        // ── Bitness-based binary resolution ──
        try
        {
            string procDumpDir = Path.GetDirectoryName(cfg.ProcDumpPath) ?? AppPaths.InstallDir;
            var bitnessResult = ProcDumpBitnessResolver.Resolve(cfg.TargetName, procDumpDir);
            Logger.Log("Monitor", $"Bitness: {bitnessResult.Summary}");

            if (!string.IsNullOrEmpty(bitnessResult.ActualBinary) && File.Exists(bitnessResult.ActualBinary))
            {
                if (!bitnessResult.ActualBinary.Equals(cfg.ProcDumpPath, StringComparison.OrdinalIgnoreCase))
                {
                    Logger.Log("Monitor", $"Switching ProcDump binary: {cfg.ProcDumpPath} → {bitnessResult.ActualBinary}");
                    cfg.ProcDumpPath = bitnessResult.ActualBinary;
                }
            }

            if (bitnessResult.Warning != null)
                Logger.Log("Monitor", $"Bitness WARNING: {bitnessResult.Warning}");
        }
        catch (Exception ex)
        {
            Logger.Log("Monitor", $"Bitness detection failed (non-fatal): {ex.Message}");
        }

        Logger.Log("Monitor", $"ProcDump: {cfg.ProcDumpPath}");
        Logger.Log("Monitor", $"DumpDir: {cfg.DumpDirectory}");
        Logger.Log("Monitor", $"Args: {cfg.BuildProcDumpArgs()}");

        // Ensure dump directory exists
        if (!Directory.Exists(cfg.DumpDirectory))
        {
            try { Directory.CreateDirectory(cfg.DumpDirectory); }
            catch (Exception ex)
            {
                Logger.Log("Monitor", $"Cannot create dump directory: {ex.Message}");
                return;
            }
        }

        Console.CancelKeyPress += (_, e) =>
        {
            e.Cancel = true;
            _stopping = true;
            Logger.Log("Monitor", "Stop signal received.");
        };

        using var notifyQueue = new NotificationQueue();

        while (!_stopping)
        {
            DateTime cycleStart = DateTime.UtcNow;
            _health.LastCycleUtc = cycleStart.ToString("O");
            _health.LastError = "";
            _health.DiskSpaceLow = false;

            Logger.Log("Monitor", "── Cycle start ──");

            try
            {
                // Disk-space guard
                if (cfg.MinFreeDiskMB > 0)
                {
                    var (ok, freeMB) = DiskSpaceGuard.CheckFreeSpace(cfg.DumpDirectory, cfg.MinFreeDiskMB);
                    _health.FreeDiskMB = freeMB;
                    _health.DiskSpaceLow = !ok;

                    if (!ok)
                    {
                        string warnMsg = $"Skipping cycle -- only {freeMB} MB free on {Path.GetPathRoot(cfg.DumpDirectory)} (threshold: {cfg.MinFreeDiskMB} MB)";
                        Logger.Log("Monitor", warnMsg);

                        // Rate-limited low-disk notification (once per hour)
                        if ((DateTime.UtcNow - _lastLowDiskNotifyUtc).TotalHours >= 1)
                        {
                            _lastLowDiskNotifyUtc = DateTime.UtcNow;
                            notifyQueue.EnqueueWarning(cfg, Notifiers,
                                $"[ProcDump] Low disk warning on {Environment.MachineName}",
                                warnMsg);
                        }

                        _health.NextRetryUtc = DateTime.UtcNow.AddSeconds(cfg.RestartDelaySeconds).ToString("O");
                        HealthWriter.Write(_health);
                        WaitBeforeRestart(cfg.RestartDelaySeconds);
                        continue;
                    }
                }

                // Dump retention / auto-cleanup
                RetentionPolicy.Apply(cfg.DumpDirectory, cfg.DumpRetentionDays, cfg.DumpRetentionMaxGB);

                // Launch ProcDump cycle
                RunProcDumpCycle(cfg, cycleStart, notifyQueue);
            }
            catch (Exception ex)
            {
                _health.LastError = ex.Message;
                Logger.Log("Monitor", $"Cycle error: {ex.Message}");
            }

            _health.NextRetryUtc = DateTime.UtcNow.AddSeconds(cfg.RestartDelaySeconds).ToString("O");
            HealthWriter.Write(_health);

            if (_stopping) break;

            WaitBeforeRestart(cfg.RestartDelaySeconds);
        }

        Logger.Log("Monitor", "ProcDump Monitor stopped.");
    }

    private static void RunProcDumpCycle(Config cfg, DateTime cycleStart, NotificationQueue notifyQueue)
    {
        var psi = new ProcessStartInfo
        {
            FileName = cfg.ProcDumpPath,
            Arguments = cfg.BuildProcDumpArgs(),
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            WorkingDirectory = cfg.DumpDirectory
        };

        Logger.Log("Monitor", $"Executing: \"{psi.FileName}\" {psi.Arguments}");

        using var proc = new Process { StartInfo = psi };

        proc.OutputDataReceived += (_, e) =>
        {
            if (e.Data != null) Logger.Log("ProcDump", e.Data);
        };
        proc.ErrorDataReceived += (_, e) =>
        {
            if (e.Data != null) Logger.Log("ProcDump-ERR", e.Data);
        };

        proc.Start();
        _health.ProcDumpPid = proc.Id;
        proc.BeginOutputReadLine();
        proc.BeginErrorReadLine();

        // Wait for ProcDump to exit, but keep health heartbeat alive so
        // external monitors can distinguish "waiting for target" from "stalled"
        int heartbeatCounter = 0;
        while (!proc.HasExited && !_stopping)
        {
            proc.WaitForExit(1000);

            if (++heartbeatCounter % 30 == 0)
            {
                _health.LastCycleUtc = DateTime.UtcNow.ToString("O");
                _health.LastError = "";
                HealthWriter.Write(_health);
            }
        }

        if (!proc.HasExited)
        {
            try { proc.Kill(true); } catch { /* best effort */ }
        }

        _health.ProcDumpPid = 0;
        int exitCode = proc.HasExited ? proc.ExitCode : -1;
        _health.LastProcDumpExitCode = exitCode;
        Logger.Log("Monitor", $"ProcDump exited with code {exitCode}.");

        // Detect newest .dmp file created after cycle start
        DetectAndNotify(cfg, cycleStart, notifyQueue);
    }

    private static void DetectAndNotify(Config cfg, DateTime cycleStart, NotificationQueue notifyQueue)
    {
        try
        {
            var dumpDir = new DirectoryInfo(cfg.DumpDirectory);
            if (!dumpDir.Exists) return;

            var newest = dumpDir
                .GetFiles("*.dmp")
                .Where(f => f.LastWriteTimeUtc >= cycleStart)
                .OrderByDescending(f => f.LastWriteTimeUtc)
                .FirstOrDefault();

            if (newest != null)
            {
                Logger.Log("Monitor", $"New dump detected: {newest.FullName}. Checking stability…");

                // ── Dump stability check (F1) ──
                bool stable = DumpStabilityChecker.WaitForStableFile(
                    newest.FullName,
                    cfg.DumpStabilityTimeoutSeconds,
                    cfg.DumpStabilityPollSeconds);

                if (!stable)
                {
                    Logger.Log("Monitor", "Dump file still locked — skipping notification.");
                    _health.LastError = "Dump file still locked after timeout — notification suppressed.";
                    return;
                }

                _health.LastDumpFileName = newest.Name;
                _health.TotalDumpCount++;

                Logger.Log("Monitor",
                    $"Dump stable: {newest.FullName} ({newest.Length / 1024.0 / 1024.0:F1} MB)");

                // ── Deduplication check ──
                if (_health.LastNotifiedDumpFile == newest.Name)
                {
                    Logger.Log("Monitor", "Dump already notified — skipping duplicate notification.");
                    return;
                }

                // Notify via all enabled channels (non-blocking)
                notifyQueue.EnqueueDump(cfg, Notifiers, newest.FullName);

                _health.LastNotifiedDumpFile = newest.Name;
                _health.LastNotifiedUtc = DateTime.UtcNow.ToString("O");
            }
            else
            {
                Logger.Log("Monitor", "No new dump file detected in this cycle.");
            }
        }
        catch (Exception ex)
        {
            Logger.Log("Monitor", $"Dump detection error: {ex.Message}");
        }
    }

    private static void WaitBeforeRestart(int delaySeconds)
    {
        if (_stopping) return;
        Logger.Log("Monitor", $"Sleeping {delaySeconds}s before restart…");
        for (int i = 0; i < delaySeconds * 10 && !_stopping; i++)
            Thread.Sleep(100);
    }
}

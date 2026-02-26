using System.Diagnostics;

namespace ProcDumpMonitor;

/// <summary>
/// Runs in --monitor mode: infinite loop that launches ProcDump, waits for it,
/// detects new .dmp files, emails, and restarts.
/// </summary>
public static class ProcDumpMonitorLoop
{
    private static volatile bool _stopping;

    public static void Run(Config cfg)
    {
        Logger.Log("Monitor", "ProcDump Monitor started.");
        Logger.Log("Monitor", $"Target: {cfg.TargetName}");
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

        while (!_stopping)
        {
            DateTime cycleStart = DateTime.UtcNow;
            Logger.Log("Monitor", "Launching ProcDump cycle...");

            try
            {
                RunProcDumpCycle(cfg, cycleStart);
            }
            catch (Exception ex)
            {
                Logger.Log("Monitor", $"Cycle error: {ex.Message}");
            }

            if (_stopping) break;

            Logger.Log("Monitor", $"Sleeping {cfg.RestartDelaySeconds}s before restart...");
            for (int i = 0; i < cfg.RestartDelaySeconds * 10 && !_stopping; i++)
                Thread.Sleep(100);
        }

        Logger.Log("Monitor", "ProcDump Monitor stopped.");
    }

    private static void RunProcDumpCycle(Config cfg, DateTime cycleStart)
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
        proc.BeginOutputReadLine();
        proc.BeginErrorReadLine();

        // Wait for ProcDump to exit (it exits after -n dumps or target exits)
        while (!proc.HasExited && !_stopping)
        {
            proc.WaitForExit(1000);
        }

        if (!proc.HasExited)
        {
            try { proc.Kill(true); } catch { /* best effort */ }
        }

        Logger.Log("Monitor", $"ProcDump exited with code {(proc.HasExited ? proc.ExitCode : -1)}.");

        // Detect newest .dmp file created after cycle start
        DetectAndNotify(cfg, cycleStart);
    }

    private static void DetectAndNotify(Config cfg, DateTime cycleStart)
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
                Logger.Log("Monitor", $"New dump detected: {newest.FullName} ({newest.Length / 1024.0 / 1024.0:F1} MB)");

                if (cfg.EmailEnabled)
                {
                    try
                    {
                        EmailNotifier.SendDumpNotification(cfg, newest.FullName);
                        Logger.Log("Monitor", "Email notification sent.");
                    }
                    catch (Exception ex)
                    {
                        Logger.Log("Monitor", $"Email send failed: {ex.Message}");
                    }
                }
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
}

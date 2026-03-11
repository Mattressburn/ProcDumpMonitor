namespace ProcDumpMonitor;

/// <summary>Simple thread-safe file logger with rotation. Never writes passwords.</summary>
public static class Logger
{
    private static readonly object Lock = new();
    private static string? _logPath;

    /// <summary>Max log file size in MB before rotation (0 = no rotation).</summary>
    public static int MaxLogSizeMB { get; set; } = 10;

    /// <summary>Number of rotated log files to keep (e.g. .1, .2, .3).</summary>
    public static int MaxLogFiles { get; set; } = 5;

    public static string LogDirectory => AppPaths.LogDir;

    public static string LogPath
    {
        get
        {
            _logPath ??= AppPaths.LogPath;
            return _logPath;
        }
    }

    public static void Log(string message)
    {
        string line = $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {message}";
        lock (Lock)
        {
            try
            {
                RotateIfNeeded();
                File.AppendAllText(LogPath, line + Environment.NewLine);
            }
            catch
            {
                // Swallow – logging must never crash the monitor.
            }
        }
    }

    public static void Log(string category, string message)
    {
        Log($"[{category}] {message}");
    }

    /// <summary>
    /// Rotate log files when the current file exceeds MaxLogSizeMB.
    /// procdump.log → procdump.log.1 → .2 → ... → .N (oldest deleted).
    /// Called inside the lock; must never throw.
    /// </summary>
    private static void RotateIfNeeded()
    {
        try
        {
            if (MaxLogSizeMB <= 0 || MaxLogFiles <= 0)
                return;

            var fi = new FileInfo(LogPath);
            if (!fi.Exists || fi.Length < MaxLogSizeMB * 1024L * 1024L)
                return;

            // Delete the oldest file beyond the retention count
            string oldest = LogPath + "." + MaxLogFiles;
            if (File.Exists(oldest))
                File.Delete(oldest);

            // Shift existing numbered files: .4 → .5, .3 → .4, …
            for (int i = MaxLogFiles - 1; i >= 1; i--)
            {
                string src = LogPath + "." + i;
                string dst = LogPath + "." + (i + 1);
                if (File.Exists(src))
                    File.Move(src, dst, overwrite: true);
            }

            // Rename current log to .1
            File.Move(LogPath, LogPath + ".1", overwrite: true);
        }
        catch
        {
            // Rotation failure must never block logging.
        }
    }
}

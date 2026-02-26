namespace ProcDumpMonitor;

/// <summary>Simple thread-safe file logger. Never writes passwords.</summary>
public static class Logger
{
    private static readonly object Lock = new();
    private static string? _logPath;

    public static string LogDirectory
    {
        get
        {
            string dir = Path.Combine(AppContext.BaseDirectory, "Logs");
            if (!Directory.Exists(dir))
                Directory.CreateDirectory(dir);
            return dir;
        }
    }

    public static string LogPath
    {
        get
        {
            _logPath ??= Path.Combine(LogDirectory, "procdump.log");
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
}

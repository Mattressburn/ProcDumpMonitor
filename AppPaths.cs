namespace ProcDumpMonitor;

/// <summary>
/// Canonical path resolution for all file system locations used by the application.
/// Handles single-file publish, portable folder copy, and normal Debug/Release layouts.
///
/// In single-file publish, <see cref="AppContext.BaseDirectory"/> may point to a temporary
/// extraction directory. <see cref="Environment.ProcessPath"/> always returns the real
/// executable path, so we derive the install directory from it.
/// </summary>
public static class AppPaths
{
    private static string? _installDir;

    /// <summary>
    /// The directory that contains ProcDumpMonitor.exe (the real, on-disk location).
    /// All portable data files (config.json, health.json, Logs/) live here.
    /// </summary>
    public static string InstallDir
    {
        get
        {
            if (_installDir is not null)
                return _installDir;

            // Environment.ProcessPath is the true path to the running EXE, even
            // when published as a single-file bundle.
            string? exePath = Environment.ProcessPath;
            if (!string.IsNullOrEmpty(exePath))
            {
                _installDir = Path.GetDirectoryName(exePath) ?? AppContext.BaseDirectory;
            }
            else
            {
                // Fallback (should not happen on .NET 8 Windows)
                _installDir = AppContext.BaseDirectory;
            }

            return _installDir;
        }
    }

    /// <summary>Path to config.json next to the executable.</summary>
    public static string ConfigPath => Path.Combine(InstallDir, "config.json");

    /// <summary>Path to health.json next to the executable.</summary>
    public static string HealthPath => Path.Combine(InstallDir, "health.json");

    /// <summary>Log directory (created on first access).</summary>
    public static string LogDir
    {
        get
        {
            string dir = Path.Combine(InstallDir, "Logs");
            if (!Directory.Exists(dir))
            {
                try { Directory.CreateDirectory(dir); }
                catch { /* best effort */ }
            }
            return dir;
        }
    }

    /// <summary>Full path to the main log file.</summary>
    public static string LogPath => Path.Combine(LogDir, "procdump.log");

    /// <summary>Full path to the running executable.</summary>
    public static string ExePath =>
        Environment.ProcessPath ?? Path.Combine(InstallDir, "ProcDumpMonitor.exe");

    /// <summary>
    /// Override the install directory for tests or CLI scenarios where the EXE
    /// path does not reflect the desired data directory.
    /// </summary>
    internal static void SetInstallDir(string dir) => _installDir = dir;
}

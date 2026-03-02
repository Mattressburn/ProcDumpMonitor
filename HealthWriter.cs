using System.Text.Json;
using System.Text.Json.Serialization;

namespace ProcDumpMonitor;

/// <summary>
/// POCO written to health.json each cycle. External monitoring tools can
/// poll this file to detect a stalled monitor without parsing logs.
/// Also stores notification-deduplication state to avoid duplicate emails.
/// </summary>
public class HealthStatus
{
    public int MonitorPid { get; set; }
    public int ProcDumpPid { get; set; }
    public string LastCycleUtc { get; set; } = "";
    public int LastProcDumpExitCode { get; set; }
    public string LastDumpFileName { get; set; } = "";
    public int TotalDumpCount { get; set; }
    public string LastError { get; set; } = "";
    public string NextRetryUtc { get; set; } = "";

    // Notification deduplication
    public string LastNotifiedDumpFile { get; set; } = "";
    public string LastNotifiedUtc { get; set; } = "";

    // Disk space snapshot
    public bool DiskSpaceLow { get; set; }
    public long FreeDiskMB { get; set; }

    public string Version { get; set; } = "";
}

/// <summary>
/// Atomically writes health.json (write-temp + rename) so a monitoring
/// tool never reads a half-written file.
/// </summary>
public static class HealthWriter
{
    private static readonly object Lock = new();

    public static string HealthPath =>
        Path.Combine(AppContext.BaseDirectory, "health.json");

    /// <summary>Write health status atomically (temp file + replace).</summary>
    public static void Write(HealthStatus status)
    {
        lock (Lock)
        {
            try
            {
                string json = JsonSerializer.Serialize(status, AppJsonContext.Default.HealthStatus);
                string tempPath = HealthPath + ".tmp";
                File.WriteAllText(tempPath, json);
                File.Move(tempPath, HealthPath, overwrite: true);
            }
            catch (Exception ex)
            {
                // Health file write must never crash the monitor.
                Logger.Log("Health", $"Failed to write health.json: {ex.Message}");
            }
        }
    }

    /// <summary>Load previous health state (e.g. to resume TotalDumpCount).</summary>
    public static HealthStatus Load()
    {
        try
        {
            if (!File.Exists(HealthPath))
                return new HealthStatus();

            string json = File.ReadAllText(HealthPath);
            return JsonSerializer.Deserialize(json, AppJsonContext.Default.HealthStatus)
                ?? new HealthStatus();
        }
        catch
        {
            return new HealthStatus();
        }
    }
}

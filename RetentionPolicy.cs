namespace ProcDumpMonitor;

/// <summary>
/// Cleans up old dump files by age and/or total folder size.
/// Two policies (both optional, applied in order):
///   1. Age-based: delete dumps older than N days.
///   2. Size-based: delete oldest dumps when total size exceeds a cap.
/// </summary>
public static class RetentionPolicy
{
    /// <summary>
    /// Apply retention policies. Both parameters are optional (0 = disabled).
    /// Returns the number of files deleted.
    /// </summary>
    public static int Apply(string dumpDirectory, int retentionDays, double maxGB)
    {
        if (retentionDays <= 0 && maxGB <= 0)
            return 0;

        if (string.IsNullOrEmpty(dumpDirectory) || !Directory.Exists(dumpDirectory))
            return 0;

        int deleted = 0;

        try
        {
            var files = new DirectoryInfo(dumpDirectory)
                .GetFiles("*.dmp")
                .OrderBy(f => f.LastWriteTimeUtc)
                .ToList();

            // ── Policy 1: Age-based retention ──
            if (retentionDays > 0)
            {
                var cutoff = DateTime.UtcNow.AddDays(-retentionDays);
                foreach (var f in files.Where(f => f.LastWriteTimeUtc < cutoff).ToList())
                {
                    try
                    {
                        f.Delete();
                        files.Remove(f);
                        deleted++;
                        Logger.Log("Retention", $"Deleted aged dump ({retentionDays}d policy): {f.Name}");
                    }
                    catch (Exception ex)
                    {
                        Logger.Log("Retention", $"Cannot delete {f.Name}: {ex.Message}");
                    }
                }
            }

            // ── Policy 2: Size-based retention ──
            if (maxGB > 0)
            {
                long maxBytes = (long)(maxGB * 1024 * 1024 * 1024);
                long totalSize = files.Sum(f =>
                {
                    try { return f.Length; }
                    catch { return 0L; }
                });

                // Delete oldest first until under the cap
                foreach (var f in files.ToList())
                {
                    if (totalSize <= maxBytes)
                        break;

                    try
                    {
                        long size = f.Length;
                        f.Delete();
                        totalSize -= size;
                        deleted++;
                        Logger.Log("Retention", $"Deleted dump (over {maxGB:F1} GB cap): {f.Name}");
                    }
                    catch (Exception ex)
                    {
                        Logger.Log("Retention", $"Cannot delete {f.Name}: {ex.Message}");
                    }
                }
            }

            if (deleted > 0)
                Logger.Log("Retention", $"Retention cleanup removed {deleted} dump file(s).");
        }
        catch (Exception ex)
        {
            Logger.Log("Retention", $"Retention policy error: {ex.Message}");
        }

        return deleted;
    }
}

namespace ProcDumpMonitor;

/// <summary>
/// Checks free disk space on the dump volume before launching ProcDump.
/// Prevents filling a disk with multi-GB full-memory dumps.
/// </summary>
public static class DiskSpaceGuard
{
    /// <summary>
    /// Returns (true, freeMB) if free space >= minFreeMB, or (false, freeMB) if below.
    /// If minFreeMB &lt;= 0 the guard is disabled and always returns (true, 0).
    /// On error, fails open (returns true) to avoid blocking dump capture.
    /// </summary>
    public static (bool Ok, long ActualFreeMB) CheckFreeSpace(string path, long minFreeMB)
    {
        if (minFreeMB <= 0)
            return (true, 0);

        try
        {
            string? root = Path.GetPathRoot(path);
            if (string.IsNullOrEmpty(root))
                return (true, -1);

            var drive = new DriveInfo(root);
            long freeMB = drive.AvailableFreeSpace / (1024L * 1024L);
            return (freeMB >= minFreeMB, freeMB);
        }
        catch (Exception ex)
        {
            Logger.Log("DiskGuard", $"Cannot check free space: {ex.Message}");
            return (true, -1); // fail open
        }
    }
}

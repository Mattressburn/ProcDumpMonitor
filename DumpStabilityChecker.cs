namespace ProcDumpMonitor;

/// <summary>
/// Waits for a dump file to become stable (size unchanged + exclusive access)
/// before allowing notifications. Prevents emailing about partial/corrupt dumps.
/// </summary>
public static class DumpStabilityChecker
{
    /// <summary>
    /// Poll until the file size is stable across two consecutive polls AND the file
    /// can be opened with exclusive access (FileShare.None).
    /// Returns true if stable, false if timeout expired.
    /// </summary>
    public static bool WaitForStableFile(string filePath, int timeoutSeconds = 30, int pollIntervalSeconds = 2)
    {
        if (string.IsNullOrEmpty(filePath) || !File.Exists(filePath))
            return false;

        if (timeoutSeconds <= 0) timeoutSeconds = 30;
        if (pollIntervalSeconds <= 0) pollIntervalSeconds = 2;

        var deadline = DateTime.UtcNow.AddSeconds(timeoutSeconds);
        long lastSize = -1;
        int stablePolls = 0;

        while (DateTime.UtcNow < deadline)
        {
            try
            {
                var fi = new FileInfo(filePath);
                if (!fi.Exists)
                    return false;

                long currentSize = fi.Length;

                // Check size stability: same for 2 consecutive polls
                if (currentSize == lastSize && currentSize > 0)
                    stablePolls++;
                else
                    stablePolls = 0;

                lastSize = currentSize;

                if (stablePolls >= 1)
                {
                    // Size is stable — now try exclusive access
                    try
                    {
                        using var fs = new FileStream(filePath, FileMode.Open, FileAccess.Read, FileShare.None);
                        Logger.Log("Stability", $"File stable: {Path.GetFileName(filePath)} ({currentSize / 1024.0 / 1024.0:F1} MB)");
                        return true;
                    }
                    catch (IOException)
                    {
                        // Still locked by another process, keep polling
                        Logger.Log("Stability", "File size stable but still locked — retrying…");
                    }
                }
            }
            catch (Exception ex)
            {
                Logger.Log("Stability", $"Error checking file: {ex.Message}");
            }

            Thread.Sleep(pollIntervalSeconds * 1000);
        }

        Logger.Log("Stability", $"Timeout ({timeoutSeconds}s) waiting for stable file: {Path.GetFileName(filePath)}");
        return false;
    }
}

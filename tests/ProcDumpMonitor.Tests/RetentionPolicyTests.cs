namespace ProcDumpMonitor.Tests;

public class RetentionPolicyTests
{
    // ── e3: Retention policy deletes old dumps ──

    [Fact]
    public void RetentionPolicy_DeletesFilesByAge()
    {
        string dir = Path.Combine(Path.GetTempPath(), $"PDM_Retention_{Guid.NewGuid():N}");
        Directory.CreateDirectory(dir);

        try
        {
            // Create a file "older than 1 day"
            string oldFile = Path.Combine(dir, "old.dmp");
            File.WriteAllText(oldFile, "old");
            File.SetLastWriteTimeUtc(oldFile, DateTime.UtcNow.AddDays(-5));

            // Create a recent file
            string newFile = Path.Combine(dir, "new.dmp");
            File.WriteAllText(newFile, "new");

            int deleted = RetentionPolicy.Apply(dir, retentionDays: 2, maxGB: 0);

            Assert.Equal(1, deleted);
            Assert.False(File.Exists(oldFile));
            Assert.True(File.Exists(newFile));
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void RetentionPolicy_DeletesOldestWhenOverSizeCap()
    {
        string dir = Path.Combine(Path.GetTempPath(), $"PDM_Retention_{Guid.NewGuid():N}");
        Directory.CreateDirectory(dir);

        try
        {
            // Create two 512 KB files: oldest should be deleted first
            string file1 = Path.Combine(dir, "first.dmp");
            File.WriteAllBytes(file1, new byte[512 * 1024]);
            File.SetLastWriteTimeUtc(file1, DateTime.UtcNow.AddHours(-2));

            string file2 = Path.Combine(dir, "second.dmp");
            File.WriteAllBytes(file2, new byte[512 * 1024]);

            // Cap at ~614 KB — total is 1024 KB, so oldest should be removed
            double maxGB = 0.0006; // ~614 KB
            int deleted = RetentionPolicy.Apply(dir, retentionDays: 0, maxGB: maxGB);

            Assert.True(deleted >= 1);
            Assert.False(File.Exists(file1));
            Assert.True(File.Exists(file2));
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    // ── e4: DiskSpaceGuard fails open ──

    [Fact]
    public void DiskSpaceGuard_FailsOpen_OnInvalidPath()
    {
        var (ok, _) = DiskSpaceGuard.CheckFreeSpace(@"Z:\NonExistent\Path", 5120);
        // Should fail open (return true) so monitoring is not blocked
        Assert.True(ok);
    }

    [Fact]
    public void DiskSpaceGuard_Disabled_WhenThresholdIsZero()
    {
        var (ok, freeMB) = DiskSpaceGuard.CheckFreeSpace(@"C:\", 0);
        Assert.True(ok);
        Assert.Equal(0, freeMB);
    }
}

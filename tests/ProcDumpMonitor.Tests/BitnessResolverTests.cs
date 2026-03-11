namespace ProcDumpMonitor.Tests;

public class BitnessResolverTests
{
    [Fact]
    public void SelectBinary_X64_Prefers_procdump64()
    {
        // Create temp dir with both binaries
        string dir = CreateTempProcDumpDir(has32: true, has64: true);
        try
        {
            var result = ProcDumpBitnessResolver.SelectBinary(TargetBitness.X64, dir);

            Assert.Equal(TargetBitness.X64, result.Bitness);
            Assert.Contains("procdump64.exe", result.ActualBinary);
            Assert.Null(result.Warning);
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void SelectBinary_X86_Prefers_procdump()
    {
        string dir = CreateTempProcDumpDir(has32: true, has64: true);
        try
        {
            var result = ProcDumpBitnessResolver.SelectBinary(TargetBitness.X86, dir);

            Assert.Equal(TargetBitness.X86, result.Bitness);
            if (Environment.Is64BitOperatingSystem)
            {
                Assert.Contains("procdump.exe", result.ActualBinary);
                Assert.DoesNotContain("procdump64", result.ActualBinary);
            }
            Assert.Null(result.Warning);
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void SelectBinary_X64_Fallback_When64Missing()
    {
        string dir = CreateTempProcDumpDir(has32: true, has64: false);
        try
        {
            var result = ProcDumpBitnessResolver.SelectBinary(TargetBitness.X64, dir);

            if (Environment.Is64BitOperatingSystem)
            {
                Assert.Contains("procdump.exe", result.ActualBinary);
                Assert.NotNull(result.Warning);
                Assert.Contains("falling back", result.Warning!, StringComparison.OrdinalIgnoreCase);
            }
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void SelectBinary_X86_Fallback_When32Missing()
    {
        string dir = CreateTempProcDumpDir(has32: false, has64: true);
        try
        {
            var result = ProcDumpBitnessResolver.SelectBinary(TargetBitness.X86, dir);

            if (Environment.Is64BitOperatingSystem)
            {
                Assert.Contains("procdump64.exe", result.ActualBinary);
                Assert.NotNull(result.Warning);
                Assert.Contains("falling back", result.Warning!, StringComparison.OrdinalIgnoreCase);
            }
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void SelectBinary_Unknown_Defaults_To64OnX64OS()
    {
        string dir = CreateTempProcDumpDir(has32: true, has64: true);
        try
        {
            var result = ProcDumpBitnessResolver.SelectBinary(TargetBitness.Unknown, dir);

            if (Environment.Is64BitOperatingSystem)
            {
                Assert.Contains("procdump64.exe", result.ActualBinary);
            }
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void SelectBinary_NoBinariesFound_ReturnsWarning()
    {
        string dir = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString());
        Directory.CreateDirectory(dir);
        try
        {
            var result = ProcDumpBitnessResolver.SelectBinary(TargetBitness.X64, dir);

            Assert.Empty(result.ActualBinary);
            Assert.NotNull(result.Warning);
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void DetectProcessBitness_NonExistentProcess_ReturnsUnknown()
    {
        var bitness = ProcDumpBitnessResolver.DetectProcessBitness("ThisProcessDefinitelyDoesNotExist_12345");
        Assert.Equal(TargetBitness.Unknown, bitness);
    }

    [Fact]
    public void Validator_ConflictingCpuThresholds()
    {
        var cfg = new Config
        {
            ProcDumpPath = "procdump.exe",
            DumpDirectory = @"C:\Dumps",
            CpuThreshold = 80,
            CpuLowThreshold = 20
        };
        // We can't do File.Exists in these tests, so test the logic parts
        var result = ProcDumpOptionsValidator.Validate(cfg);
        Assert.Contains(result.Errors, e => e.Contains("Cannot specify both"));
    }

    [Fact]
    public void Validator_CpuDurationWithoutThreshold()
    {
        var cfg = new Config
        {
            ProcDumpPath = "procdump.exe",
            DumpDirectory = @"C:\Dumps",
            CpuDurationSeconds = 10
        };
        var result = ProcDumpOptionsValidator.Validate(cfg);
        Assert.Contains(result.Errors, e => e.Contains("CPU duration"));
    }

    [Fact]
    public void Validator_NoTrigger_ProducesWarning()
    {
        var cfg = new Config
        {
            ProcDumpPath = "procdump.exe",
            DumpDirectory = @"C:\Dumps",
            DumpOnException = false,
            DumpOnTerminate = false,
            UseClone = false
        };
        var result = ProcDumpOptionsValidator.Validate(cfg);
        Assert.Contains(result.Warnings, w => w.Contains("No trigger"));
    }

    private static string CreateTempProcDumpDir(bool has32, bool has64)
    {
        string dir = Path.Combine(Path.GetTempPath(), "ProcDumpTest_" + Guid.NewGuid().ToString("N")[..8]);
        Directory.CreateDirectory(dir);

        if (has32) File.WriteAllText(Path.Combine(dir, "procdump.exe"), "fake32");
        if (has64) File.WriteAllText(Path.Combine(dir, "procdump64.exe"), "fake64");

        return dir;
    }
}

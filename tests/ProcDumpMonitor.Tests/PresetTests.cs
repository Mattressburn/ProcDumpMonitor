namespace ProcDumpMonitor.Tests;

public class PresetTests
{
    [Fact]
    public void LowImpactPreset_ProducesExpectedFlags()
    {
        var cfg = MakeBaseConfig();
        var preset = ProcDumpPreset.FindByName("Low impact full dump");
        Assert.NotNull(preset);

        preset!.Apply(cfg);
        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-accepteula", args);
        Assert.Contains("-ma", args);
        Assert.Contains("-a", args);
        Assert.Contains("-r", args);
        Assert.Contains("-n 1", args);
    }

    [Fact]
    public void CrashCapturePreset_ProducesExpectedFlags()
    {
        var cfg = MakeBaseConfig();
        var preset = ProcDumpPreset.FindByName("Crash capture");
        Assert.NotNull(preset);

        preset!.Apply(cfg);
        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-ma", args);
        Assert.Contains("-e", args);
        Assert.Contains("-t", args);
        // Should NOT have -a or -r (reset by preset)
        Assert.DoesNotContain(" -a", args);
        Assert.DoesNotContain(" -r", args);
    }

    [Fact]
    public void HangCapturePreset_ProducesExpectedFlags()
    {
        var cfg = MakeBaseConfig();
        var preset = ProcDumpPreset.FindByName("Hang capture");
        Assert.NotNull(preset);

        preset!.Apply(cfg);
        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-ma", args);
        Assert.Contains("-h", args);
        Assert.DoesNotContain("-e", args);
        Assert.DoesNotContain("-t", args);
    }

    [Fact]
    public void HighCpuPreset_ProducesExpectedFlags()
    {
        var cfg = MakeBaseConfig();
        var preset = ProcDumpPreset.FindByName("High CPU spike capture");
        Assert.NotNull(preset);

        preset!.Apply(cfg);
        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-ma", args);
        Assert.Contains("-c 90", args);
        Assert.Contains("-s 10", args);
        Assert.Contains("-n 3", args);
    }

    [Fact]
    public void MemoryThresholdPreset_ProducesExpectedFlags()
    {
        var cfg = MakeBaseConfig();
        var preset = ProcDumpPreset.FindByName("Memory threshold capture");
        Assert.NotNull(preset);

        preset!.Apply(cfg);
        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-ma", args);
        Assert.Contains("-m 2048", args);
        Assert.Contains("-n 3", args);
    }

    [Fact]
    public void Preset_ResetsExistingTriggers()
    {
        // Start with a config that has CPU and exception triggers
        var cfg = MakeBaseConfig();
        cfg.DumpOnException = true;
        cfg.CpuThreshold = 50;

        var preset = ProcDumpPreset.FindByName("Hang capture");
        Assert.NotNull(preset);

        preset!.Apply(cfg);

        // CPU and exception should be reset to off
        Assert.Equal(0, cfg.CpuThreshold);
        Assert.False(cfg.DumpOnException);
        Assert.True(cfg.HangWindowSeconds > 0);
    }

    [Fact]
    public void Preset_PreservesPathAndTarget()
    {
        var cfg = MakeBaseConfig();
        cfg.ProcDumpPath = @"C:\Tools\procdump64.exe";
        cfg.DumpDirectory = @"C:\Dumps";
        cfg.TargetName = "MyService";

        var preset = ProcDumpPreset.FindByName("Crash capture");
        Assert.NotNull(preset);

        preset!.Apply(cfg);

        // Path, directory, target should be unchanged
        Assert.Equal(@"C:\Tools\procdump64.exe", cfg.ProcDumpPath);
        Assert.Equal(@"C:\Dumps", cfg.DumpDirectory);
        Assert.Equal("MyService", cfg.TargetName);
    }

    [Fact]
    public void AllPresetsExist()
    {
        Assert.True(ProcDumpPreset.All.Count >= 5);
        foreach (var preset in ProcDumpPreset.All)
        {
            Assert.False(string.IsNullOrWhiteSpace(preset.Name));
            Assert.False(string.IsNullOrWhiteSpace(preset.Description));
            Assert.False(string.IsNullOrWhiteSpace(preset.EffectiveFlags));
        }
    }

    [Fact]
    public void Config_BuildProcDumpArgs_IncludesNewV3Flags()
    {
        var cfg = MakeBaseConfig();
        cfg.DumpType = "Full";
        cfg.DumpOnException = true;
        cfg.DumpOnTerminate = true;
        cfg.UseClone = true;
        cfg.AvoidOutage = true;
        cfg.OverwriteExisting = true;
        cfg.CpuThreshold = 80;
        cfg.CpuDurationSeconds = 5;
        cfg.CpuPerUnit = true;
        cfg.MaxDumps = 2;
        cfg.WaitForProcess = true;

        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-accepteula", args);
        Assert.Contains("-ma", args);
        Assert.Contains("-e", args);
        Assert.Contains("-t", args);
        Assert.Contains("-r", args);
        Assert.Contains("-a", args);
        Assert.Contains("-o", args);
        Assert.Contains("-c 80", args);
        Assert.Contains("-s 5", args);
        Assert.Contains("-u", args);
        Assert.Contains("-n 2", args);
        Assert.Contains("-w", args);
    }

    [Fact]
    public void Config_BuildProcDumpArgs_IncludesAdvancedFlags()
    {
        var cfg = MakeBaseConfig();
        cfg.WerIntegration = true;
        cfg.AvoidTerminateTimeout = 30;
        cfg.ExceptionFilterInclude = "OutOfMemory";
        cfg.ExceptionFilterExclude = "ThreadAbort";

        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-wer", args);
        Assert.Contains("-at 30", args);
        Assert.Contains("-f \"OutOfMemory\"", args);
        Assert.Contains("-fx \"ThreadAbort\"", args);
    }

    [Fact]
    public void Config_BuildProcDumpArgs_ThreadDumpType()
    {
        var cfg = MakeBaseConfig();
        cfg.DumpType = "ThreadDump";

        string args = cfg.BuildProcDumpArgs();
        Assert.Contains("-mt", args);
        Assert.DoesNotContain("-ma", args);
    }

    [Fact]
    public void Config_BuildProcDumpArgs_WaitForProcessFalse()
    {
        var cfg = MakeBaseConfig();
        cfg.WaitForProcess = false;

        string args = cfg.BuildProcDumpArgs();
        Assert.DoesNotContain("-w", args);
        Assert.Contains("MyApp", args);
    }

    private static Config MakeBaseConfig() => new()
    {
        TargetName = "MyApp",
        DumpDirectory = @"C:\Dumps",
        ProcDumpPath = @"C:\Tools\procdump64.exe",
        MaxDumps = 1,
        WaitForProcess = true
    };
}

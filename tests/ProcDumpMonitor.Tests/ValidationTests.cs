namespace ProcDumpMonitor.Tests;

public class ValidationTests
{
    // ── a4: TaskNameHelper sanitization ──

    [Theory]
    [InlineData("ProcDump Monitor CrossFire", "ProcDump Monitor CrossFire")]
    [InlineData("", "")]
    [InlineData("   ", "")]
    public void TaskNameHelper_Sanitize_HandlesEdgeCases(string input, string expected)
    {
        Assert.Equal(expected, TaskNameHelper.Sanitize(input));
    }

    [Fact]
    public void TaskNameHelper_Sanitize_CollapsesWhitespaceDashRuns()
    {
        // " - " is a 3-char run of [whitespace/dash] and collapses to a single space
        string result = TaskNameHelper.Sanitize("ProcDump Monitor - CrossFire");
        Assert.Equal("ProcDump Monitor CrossFire", result);
    }

    [Fact]
    public void TaskNameHelper_Sanitize_ReplacesInvalidChars()
    {
        // Invalid Task Scheduler chars: \ / : * ? " < > |
        string result = TaskNameHelper.Sanitize("Task:With*Bad<Chars>");
        Assert.DoesNotContain(":", result);
        Assert.DoesNotContain("*", result);
        Assert.DoesNotContain("<", result);
        Assert.DoesNotContain(">", result);
    }

    [Fact]
    public void TaskNameHelper_Sanitize_ReplacesSlashes()
    {
        string result = TaskNameHelper.Sanitize(@"A\B/C");
        Assert.DoesNotContain(@"\", result);
        Assert.DoesNotContain("/", result);
    }

    // ── a5: Config.BuildProcDumpArgs includes all flags ──

    [Fact]
    public void Config_BuildProcDumpArgs_IncludesExpectedFlags()
    {
        var cfg = new Config
        {
            TargetName = "MyApp",
            DumpType = "Full",
            DumpOnException = true,
            DumpOnTerminate = true,
            UseClone = true,
            MaxDumps = 3,
            DumpDirectory = @"C:\Dumps",
            CpuThreshold = 90,
            MemoryCommitMB = 0,
            HangWindowSeconds = 0
        };

        string args = cfg.BuildProcDumpArgs();

        Assert.Contains("-accepteula", args);
        Assert.Contains("-ma", args);       // Full dump
        Assert.Contains("-e", args);        // Exception trigger
        Assert.Contains("-t", args);        // Terminate trigger
        Assert.Contains("-r", args);        // Clone
        Assert.Contains("-n 3", args);      // Max dumps
        Assert.Contains("-c 90", args);     // CPU threshold
        Assert.Contains("-w MyApp", args);  // Target name
        Assert.Contains(@"""C:\Dumps""", args); // Dump directory quoted
    }

    [Fact]
    public void Config_BuildProcDumpArgs_OmitsCpuWhenZero()
    {
        var cfg = new Config { CpuThreshold = 0, DumpOnException = true, TargetName = "X", DumpDirectory = @"C:\D" };
        string args = cfg.BuildProcDumpArgs();

        Assert.DoesNotContain("-c ", args);
    }

    // ── a3: Email address validation ──

    [Theory]
    [InlineData("user@example.com", true)]
    [InlineData("user@example.com;admin@example.com", true)]
    [InlineData("not-an-email", false)]
    [InlineData("", false)]
    [InlineData("good@x.com;bad", false)]
    public void EmailNotifier_ValidateAddressList(string input, bool expectedValid)
    {
        var (valid, _) = EmailNotifier.ValidateAddressList(input, "To");
        Assert.Equal(expectedValid, valid);
    }
}

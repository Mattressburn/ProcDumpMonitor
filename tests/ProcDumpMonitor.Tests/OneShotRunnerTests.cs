using System.Diagnostics;

namespace ProcDumpMonitor.Tests;

public class OneShotRunnerTests
{
    private static Config MakeTestConfig(string? dumpDir = null) => new()
    {
        TaskName = "Test-Task",
        TargetName = "TestProcess.exe",
        ProcDumpPath = "procdump.exe",
        DumpDirectory = dumpDir ?? Path.Combine(Path.GetTempPath(), $"PDM_Test_{Guid.NewGuid():N}"),
        EmailEnabled = true,
        SmtpServer = "localhost",
        SmtpPort = 25,
        FromAddress = "test@test.com",
        ToAddress = "dest@test.com"
    };

    // ── b1: Create and remove task via fake scheduler ──

    [Fact]
    public void CreateAndRemoveTask_ViaFakeScheduler()
    {
        var cfg = MakeTestConfig();
        var taskOps = new SimulatedTaskSchedulerOps();
        var procDump = new SimulatedProcDumpRunner();
        var email = new FakeEmailSender();
        var options = new OneShotOptions { SimulateDump = true, SimulateTask = true, NoEmail = true };

        var runner = new OneShotRunner(cfg, taskOps, procDump, email, options);
        var result = runner.Execute(CancellationToken.None);

        Assert.True(result.TaskCreated);
        Assert.True(result.TaskRemoved);
        Assert.False(taskOps.TaskExists(TaskNameHelper.Sanitize(cfg.TaskName)));

        // Cleanup
        try { Directory.Delete(cfg.DumpDirectory, true); } catch { }
    }

    // ── b2: Task is cleaned up on fatal error ──

    [Fact]
    public void CleansUpTask_OnProcDumpError()
    {
        var cfg = MakeTestConfig();
        var taskOps = new SimulatedTaskSchedulerOps();
        var procDump = new ThrowingProcDumpRunner();
        var email = new FakeEmailSender();
        var options = new OneShotOptions { SimulateDump = false, SimulateTask = true, NoEmail = true };

        var runner = new OneShotRunner(cfg, taskOps, procDump, email, options);
        var result = runner.Execute(CancellationToken.None);

        // Task should still be cleaned up despite the error
        Assert.True(result.TaskCreated);
        Assert.True(result.TaskRemoved);
        Assert.False(result.Success);
        Assert.NotNull(result.Error);
    }

    // ── c1: Full dump → notify → cleanup flow ──

    [Fact]
    public void DumpThenNotifyThenCleanup_FullFlow()
    {
        var cfg = MakeTestConfig();
        var taskOps = new SimulatedTaskSchedulerOps();
        var procDump = new SimulatedProcDumpRunner();
        var email = new FakeEmailSender();
        var options = new OneShotOptions { SimulateDump = true, SimulateTask = true, NoEmail = false };

        var runner = new OneShotRunner(cfg, taskOps, procDump, email, options);
        var result = runner.Execute(CancellationToken.None);

        Assert.True(result.Success);
        Assert.NotNull(result.DumpFilePath);
        Assert.True(result.EmailSent);
        Assert.True(email.CallCount > 0);
        Assert.True(result.TaskRemoved);

        // Cleanup
        try { if (result.DumpFilePath != null) File.Delete(result.DumpFilePath); } catch { }
        try { Directory.Delete(cfg.DumpDirectory, true); } catch { }
    }

    // ── c2: Email skipped when --no-email flag is set ──

    [Fact]
    public void SkipsEmail_WhenNoEmailFlagSet()
    {
        var cfg = MakeTestConfig();
        var taskOps = new SimulatedTaskSchedulerOps();
        var procDump = new SimulatedProcDumpRunner();
        var email = new FakeEmailSender();
        var options = new OneShotOptions { SimulateDump = true, SimulateTask = true, NoEmail = true };

        var runner = new OneShotRunner(cfg, taskOps, procDump, email, options);
        var result = runner.Execute(CancellationToken.None);

        Assert.True(result.Success);
        Assert.False(result.EmailSent);
        Assert.Equal(0, email.CallCount);

        try { Directory.Delete(cfg.DumpDirectory, true); } catch { }
    }

    // ── d1: No lingering threads after one-shot ──

    [Fact]
    public void ExitsCleanly_NoPendingBackgroundThreads()
    {
        var cfg = MakeTestConfig();
        var taskOps = new SimulatedTaskSchedulerOps();
        var procDump = new SimulatedProcDumpRunner();
        var email = new FakeEmailSender();
        var options = new OneShotOptions { SimulateDump = true, SimulateTask = true, NoEmail = true };

        int threadsBefore = Process.GetCurrentProcess().Threads.Count;

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
        var runner = new OneShotRunner(cfg, taskOps, procDump, email, options);
        var result = runner.Execute(cts.Token);

        // Allow threadpool to settle
        Thread.Sleep(500);
        int threadsAfter = Process.GetCurrentProcess().Threads.Count;

        Assert.True(result.Success);
        // Threads should not grow unboundedly; allow small variance for GC/threadpool
        Assert.True(threadsAfter <= threadsBefore + 5,
            $"Thread count grew from {threadsBefore} to {threadsAfter} — possible leak");

        try { Directory.Delete(cfg.DumpDirectory, true); } catch { }
    }
}

/// <summary>Fake ProcDump runner that throws on Start() to test error cleanup paths.</summary>
public sealed class ThrowingProcDumpRunner : IProcDumpRunner
{
    public void Start(Config cfg) => throw new InvalidOperationException("Simulated ProcDump failure");
    public int WaitForCompletion(CancellationToken ct) => -1;
    public string? GetDumpOutputPath() => null;
}

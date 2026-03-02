namespace ProcDumpMonitor;

/// <summary>
/// Minimal in-process test harness for the one-shot sequence.
/// Validates the full create → dump → email → remove → exit pipeline
/// using only fakes (no real Task Scheduler, ProcDump, or SMTP).
///
/// Run from the CLI:
///   ProcDumpMonitor.exe --selftest
/// </summary>
public static class OneShotSelfTest
{
    /// <summary>
    /// Executes the one-shot flow with all-fake implementations.
    /// Returns 0 if all assertions pass, 1 if any fail.
    /// </summary>
    public static int Run()
    {
        Console.WriteLine("═══ OneShot Self-Test ═══");
        Console.WriteLine();

        int failures = 0;

        var cfg = new Config
        {
            TaskName = "ProcDump Monitor - SelfTest",
            TargetName = "SelfTest.exe",
            ProcDumpPath = "procdump.exe",
            DumpDirectory = Path.Combine(Path.GetTempPath(), "ProcDumpMonitor_SelfTest"),
            EmailEnabled = true,
            SmtpServer = "smtp.jci.com",
            SmtpPort = 25,
            FromAddress = "matthew.raburn@jci.com",
            ToAddress = "matthew.raburn@jci.com",
        };

        // All fakes
        var taskOps = new SimulatedTaskSchedulerOps();
        var procDump = new SimulatedProcDumpRunner();
        var emailSender = new FakeEmailSender();

        var options = new OneShotOptions
        {
            SimulateDump = true,
            SimulateTask = true,
            NoEmail = false
        };

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(30));
        var runner = new OneShotRunner(cfg, taskOps, procDump, emailSender, options);
        var result = runner.Execute(cts.Token);

        // Print steps
        Console.WriteLine("Steps:");
        foreach (var step in result.Steps)
            Console.WriteLine($"  {step}");
        Console.WriteLine();

        // Assertions
        failures += Assert("Success", result.Success, true);
        failures += Assert("TaskCreated", result.TaskCreated, true);
        failures += Assert("TaskRemoved", result.TaskRemoved, true);
        failures += Assert("EmailSent", result.EmailSent, true);
        failures += Assert("Simulated", result.Simulated, true);
        failures += Assert("Error is null", result.Error == null, true);
        failures += Assert("DumpFilePath not null", result.DumpFilePath != null, true);
        failures += Assert("FakeEmail.CallCount", emailSender.CallCount > 0, true);
        failures += Assert("Task no longer exists",
            !taskOps.TaskExists(TaskNameHelper.Sanitize(cfg.TaskName)), true);

        // Verify simulated dump file was created
        if (result.DumpFilePath != null)
        {
            bool exists = File.Exists(result.DumpFilePath);
            failures += Assert("Simulated dump file exists", exists, true);

            // Cleanup
            try { File.Delete(result.DumpFilePath); } catch { }
        }

        try { Directory.Delete(cfg.DumpDirectory, true); } catch { }

        Console.WriteLine();
        if (failures == 0)
        {
            Console.ForegroundColor = ConsoleColor.Green;
            Console.WriteLine("ALL TESTS PASSED.");
        }
        else
        {
            Console.ForegroundColor = ConsoleColor.Red;
            Console.WriteLine($"{failures} TEST(S) FAILED.");
        }
        Console.ResetColor();

        return failures == 0 ? 0 : 1;
    }

    private static int Assert(string name, object actual, object expected)
    {
        bool pass = Equals(actual, expected);
        if (pass)
        {
            Console.ForegroundColor = ConsoleColor.Green;
            Console.Write("  ✓ ");
        }
        else
        {
            Console.ForegroundColor = ConsoleColor.Red;
            Console.Write("  ✖ ");
        }
        Console.ResetColor();
        Console.WriteLine($"{name}: {actual} (expected {expected})");
        return pass ? 0 : 1;
    }
}

/// <summary>
/// Fake email sender for self-test. Records calls without network access.
/// </summary>
public sealed class FakeEmailSender : IEmailSender
{
    public int CallCount { get; private set; }
    public string? LastSubject { get; private set; }

    public void SendTestEmail(Config cfg)
    {
        CallCount++;
        LastSubject = $"[Test] {cfg.TargetName}";
        Logger.Log("FakeEmail", $"SendTestEmail called (to={cfg.ToAddress}).");
    }

    public void SendDumpNotification(Config cfg, string dumpFilePath)
    {
        CallCount++;
        LastSubject = $"[Dump] {cfg.TargetName}";
        Logger.Log("FakeEmail", $"SendDumpNotification called (to={cfg.ToAddress}, dump={dumpFilePath}).");
    }
}

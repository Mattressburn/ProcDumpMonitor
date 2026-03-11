using System.Text.Json;

namespace ProcDumpMonitor;

internal static class Program
{
    // CLI verbs that require Administrator privileges.
    private static readonly string[] CliVerbs =
    [
        "--monitor", "--oneshot", "--install", "--update",
        "--uninstall", "--start", "--stop", "--status",
        "--support-diagnostics", "--selftest"
    ];

    [STAThread]
    static void Main(string[] args)
    {
        bool noElevate = HasFlag(args, "--no-elevate");

        // Strip --no-elevate from the args so downstream code never sees it.
        if (noElevate)
            args = args.Where(a => !a.Equals("--no-elevate", StringComparison.OrdinalIgnoreCase)).ToArray();

        if (!noElevate && !ElevationHelper.IsElevated())
        {
            if (args.Length == 0)
            {
                // GUI launch: silently elevate via UAC prompt.
                if (ElevationHelper.RelaunchElevated(args))
                {
                    // Elevated process started; exit the non-elevated instance.
                    return;
                }
                // User cancelled UAC; exit cleanly.
                return;
            }

            // CLI launch: do not silently relaunch.
            if (args.Any(a => CliVerbs.Contains(a, StringComparer.OrdinalIgnoreCase)))
            {
                Console.Error.WriteLine("ERROR: This command requires Administrator privileges.");
                Console.Error.WriteLine("Re-run from an elevated (Administrator) prompt, or add --no-elevate to skip.");
                Environment.Exit(1);
            }
        }

        // ── No arguments: GUI mode ──
        if (args.Length == 0)
        {
            ApplicationConfiguration.Initialize();
            Application.Run(new MainForm());
            return;
        }

        // ── CLI mode (no WinForms initialization) ──

        string? configPath = GetArgValue(args, "--config");

        // Validate --config path early
        if (configPath != null && !File.Exists(configPath))
        {
            Console.Error.WriteLine($"Config file not found: {configPath}");
            Environment.Exit(2);
        }

        // --help
        if (HasFlag(args, "--help") || HasFlag(args, "-h") || HasFlag(args, "/?"))
        {
            PrintHelp();
            Environment.Exit(0);
        }

        // --version
        if (HasFlag(args, "--version"))
        {
            Console.WriteLine(typeof(Program).Assembly.GetName().Version?.ToString() ?? "1.0.0");
            Environment.Exit(0);
        }

        // --support-diagnostics [--since <ISO8601>] [--until <ISO8601>]
        if (HasFlag(args, "--support-diagnostics"))
        {
            if (!SupportDiagnosticsService.IsElevated())
            {
                Console.Error.WriteLine("ERROR: --support-diagnostics requires Administrator privileges.");
                Console.Error.WriteLine("Re-run this command from an elevated (Administrator) prompt.");
                Environment.Exit(1);
            }

            try
            {
                DateTime? since = null, until = null;
                string? sinceStr = GetArgValue(args, "--since");
                string? untilStr = GetArgValue(args, "--until");
                if (sinceStr != null) since = DateTime.Parse(sinceStr, null, System.Globalization.DateTimeStyles.RoundtripKind);
                if (untilStr != null) until = DateTime.Parse(untilStr, null, System.Globalization.DateTimeStyles.RoundtripKind);

                Console.WriteLine("Creating support diagnostics bundle…");
                string zipPath = SupportDiagnosticsService.CreateBundle(since, until, msg => Console.WriteLine($"  {msg}"));
                Console.WriteLine($"Bundle created: {zipPath}");
                Environment.Exit(0);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Diagnostics failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // --selftest
        if (HasFlag(args, "--selftest"))
        {
            Environment.Exit(OneShotSelfTest.Run());
        }

        // --monitor --config <path>
        if (HasFlag(args, "--monitor"))
        {
            var cfg = Config.Load(configPath);
            ProcDumpMonitorLoop.Run(cfg);
            return;
        }

        // --oneshot [--simulate-dump] [--no-email] [--simulate-task] [--config <path>]
        if (HasFlag(args, "--oneshot"))
        {
            try
            {
                var cfg = Config.Load(configPath);
                bool simulate = HasFlag(args, "--simulate-dump");
                bool simulateTask = HasFlag(args, "--simulate-task") || simulate;
                bool noEmail = HasFlag(args, "--no-email");

                var options = new OneShotOptions
                {
                    SimulateDump = simulate,
                    SimulateTask = simulateTask,
                    NoEmail = noEmail
                };

                ITaskSchedulerOps taskOps = simulateTask
                    ? new SimulatedTaskSchedulerOps()
                    : new RealTaskSchedulerOps();

                IProcDumpRunner procDump = simulate
                    ? new SimulatedProcDumpRunner()
                    : new RealProcDumpRunner();

                IEmailSender email = new RealEmailSender();

                using var cts = new CancellationTokenSource();
                Console.CancelKeyPress += (_, e) => { e.Cancel = true; cts.Cancel(); };

                var runner = new OneShotRunner(cfg, taskOps, procDump, email, options, configPath);
                var result = runner.Execute(cts.Token);

                // Write structured JSON result to stdout
                Console.WriteLine("──── RESULT ────");
                Console.WriteLine(JsonSerializer.Serialize(result, AppJsonContext.Default.OneShotResult));

                Environment.Exit(result.Success ? 0 : 1);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"OneShot failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // --install / --update --config <path>
        if (HasFlag(args, "--install") || HasFlag(args, "--update"))
        {
            try
            {
                var cfg = Config.Load(configPath);
                cfg.Save(); // Save migrated config to default path
                bool existed = TaskSchedulerService.InstallOrUpdate(cfg);
                Console.WriteLine(existed
                    ? $"Task '{cfg.TaskName}' updated successfully."
                    : $"Task '{cfg.TaskName}' created successfully.");
                Console.WriteLine($"Config saved to {Config.DefaultConfigPath}");
                Environment.Exit(0);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Install failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // --uninstall [--config <path>]
        if (HasFlag(args, "--uninstall"))
        {
            try
            {
                var cfg = Config.Load(configPath);
                string taskName = TaskNameHelper.Sanitize(cfg.TaskName);
                if (TaskSchedulerService.TaskExists(taskName))
                {
                    TaskSchedulerService.RemoveTask(taskName);
                    Console.WriteLine($"Task '{taskName}' removed.");
                }
                else
                {
                    Console.WriteLine($"Task '{taskName}' not found (already removed).");
                }
                Environment.Exit(0);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Uninstall failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // --start [--config <path>]
        if (HasFlag(args, "--start"))
        {
            try
            {
                var cfg = Config.Load(configPath);
                TaskSchedulerService.StartNow(cfg.TaskName);
                Console.WriteLine($"Task '{cfg.TaskName}' started.");
                Environment.Exit(0);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Start failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // --stop [--config <path>]
        if (HasFlag(args, "--stop"))
        {
            try
            {
                var cfg = Config.Load(configPath);
                TaskSchedulerService.StopTask(cfg.TaskName);
                Console.WriteLine($"Task '{cfg.TaskName}' stopped.");
                Environment.Exit(0);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Stop failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // --status [--config <path>]
        if (HasFlag(args, "--status"))
        {
            try
            {
                var cfg = Config.Load(configPath);
                var info = TaskSchedulerService.GetDetailedStatus(cfg.TaskName);
                var output = new CliStatusOutput
                {
                    TaskName = cfg.TaskName,
                    MachineName = Environment.MachineName,
                    Exists = info.Exists,
                    State = info.State,
                    LastRunTime = info.LastRunTime,
                    LastRunResult = info.LastRunResult,
                    NextRunTime = info.NextRunTime
                };
                Console.WriteLine(JsonSerializer.Serialize(output, AppJsonContext.Default.CliStatusOutput));
                Environment.Exit(0);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Status query failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // --export-config <output-path> [--config <source>]
        if (HasFlag(args, "--export-config"))
        {
            string? outputPath = GetArgValue(args, "--export-config");
            if (string.IsNullOrEmpty(outputPath))
            {
                Console.Error.WriteLine("--export-config requires an output file path.");
                Environment.Exit(2);
            }
            try
            {
                var cfg = Config.Load(configPath);
                ConfigExportImport.Export(cfg, outputPath);
                Console.WriteLine($"Config exported to {outputPath} (secrets redacted).");
                Environment.Exit(0);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Export failed: {ex.Message}");
                Environment.Exit(1);
            }
        }

        // Unknown arguments
        Console.Error.WriteLine("Unknown command. Use --help for usage information.");
        Environment.Exit(2);
    }

    // ── Argument helpers ──

    private static bool HasFlag(string[] args, string flag)
        => args.Any(a => a.Equals(flag, StringComparison.OrdinalIgnoreCase));

    private static string? GetArgValue(string[] args, string name)
    {
        for (int i = 0; i < args.Length - 1; i++)
        {
            if (args[i].Equals(name, StringComparison.OrdinalIgnoreCase))
                return args[i + 1];
        }
        return null;
    }

    private static void PrintHelp()
    {
        Console.WriteLine("""
            ProcDumpMonitor.exe [options]

            GUI (default):
              (no arguments)              Launch the WinForms configuration UI.

            Monitor (headless):
              --monitor --config <path>   Run the continuous ProcDump monitor loop.

            One-Shot (run once, then exit):
              --oneshot [--config <path>]  Create task → capture dump → email → remove task → exit.
              --simulate-dump             Use fake ProcDump and fake task (for testing).
              --simulate-task             Use fake task scheduler only (real ProcDump).
              --no-email                  Skip email sending (for dev/test).

            Task Management (silent, no GUI):
              --install --config <path>   Create or update the Scheduled Task from config.
              --update  --config <path>   Alias for --install.
              --uninstall [--config <p>]  Remove the Scheduled Task.
              --start     [--config <p>]  Start (demand-run) the Scheduled Task.
              --stop      [--config <p>]  Stop the running Scheduled Task.

            Status:
              --status    [--config <p>]  Print task status as JSON to stdout; exit 0.

            Diagnostics:
              --support-diagnostics       Create a support bundle ZIP (requires elevation).
              --since <ISO8601>           Start of time range (default: 24 hours ago).
              --until <ISO8601>           End of time range (default: now).

            Export:
              --export-config <out>       Export config with secrets redacted.

            Miscellaneous:
              --help, -h                  Print this help text and exit.
              --version                   Print assembly version and exit.
              --no-elevate                Skip automatic UAC elevation (debug/CI).

            Exit Codes:
              0  Success
              1  Operation failed (details on stderr)
              2  Bad arguments / missing required parameter
            """);
    }
}

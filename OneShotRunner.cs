using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace ProcDumpMonitor;

// ═══════════════════════════════════════════════════════════════
//  Interfaces — enable simulation and trimming safety
// ═══════════════════════════════════════════════════════════════

/// <summary>Abstraction over Windows Task Scheduler operations.</summary>
public interface ITaskSchedulerOps
{
    bool InstallOrUpdate(Config cfg);
    void StartNow(string taskName);
    void RemoveTask(string taskName);
    bool TaskExists(string taskName);
    TaskStatusInfo GetDetailedStatus(string taskName);
}

/// <summary>Abstraction over ProcDump execution and dump detection.</summary>
public interface IProcDumpRunner
{
    /// <summary>Start ProcDump (or simulation). Returns immediately.</summary>
    void Start(Config cfg);

    /// <summary>Block until ProcDump exits or dump is detected. Returns exit code.</summary>
    int WaitForCompletion(CancellationToken ct);

    /// <summary>Path to the dump file produced (or null if none).</summary>
    string? GetDumpOutputPath();
}

/// <summary>Abstraction over email sending.</summary>
public interface IEmailSender
{
    void SendTestEmail(Config cfg);
    void SendDumpNotification(Config cfg, string dumpFilePath);
}

// ═══════════════════════════════════════════════════════════════
//  Real implementations — thin wrappers over existing statics
// ═══════════════════════════════════════════════════════════════

/// <summary>Delegates to the existing static <see cref="TaskSchedulerService"/>.</summary>
public sealed class RealTaskSchedulerOps : ITaskSchedulerOps
{
    public bool InstallOrUpdate(Config cfg) => TaskSchedulerService.InstallOrUpdate(cfg);
    public void StartNow(string taskName) => TaskSchedulerService.StartNow(taskName);
    public void RemoveTask(string taskName) => TaskSchedulerService.RemoveTask(taskName);
    public bool TaskExists(string taskName) => TaskSchedulerService.TaskExists(taskName);
    public TaskStatusInfo GetDetailedStatus(string taskName) => TaskSchedulerService.GetDetailedStatus(taskName);
}

/// <summary>Delegates to the existing static <see cref="EmailNotifier"/>.</summary>
public sealed class RealEmailSender : IEmailSender
{
    public void SendTestEmail(Config cfg) => EmailNotifier.SendTestEmail(cfg);
    public void SendDumpNotification(Config cfg, string dumpFilePath) =>
        EmailNotifier.SendDumpNotification(cfg, dumpFilePath);
}

/// <summary>Launches the real procdump.exe and detects the dump file.</summary>
public sealed class RealProcDumpRunner : IProcDumpRunner
{
    private Process? _proc;
    private string? _dumpPath;
    private Config _cfg = new();

    public void Start(Config cfg)
    {
        _cfg = cfg;

        if (!Directory.Exists(cfg.DumpDirectory))
            Directory.CreateDirectory(cfg.DumpDirectory);

        var psi = new ProcessStartInfo
        {
            FileName = cfg.ProcDumpPath,
            Arguments = cfg.BuildProcDumpArgs(),
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            WorkingDirectory = cfg.DumpDirectory
        };

        Logger.Log("OneShot", $"Launching ProcDump: \"{psi.FileName}\" {psi.Arguments}");
        _proc = new Process { StartInfo = psi };
        _proc.OutputDataReceived += (_, e) => { if (e.Data != null) Logger.Log("ProcDump", e.Data); };
        _proc.ErrorDataReceived += (_, e) => { if (e.Data != null) Logger.Log("ProcDump-ERR", e.Data); };
        _proc.Start();
        _proc.BeginOutputReadLine();
        _proc.BeginErrorReadLine();
    }

    public int WaitForCompletion(CancellationToken ct)
    {
        if (_proc == null) return -1;
        try
        {
            while (!_proc.HasExited && !ct.IsCancellationRequested)
                _proc.WaitForExit(1000);

            if (!_proc.HasExited)
            {
                try { _proc.Kill(true); } catch { /* best effort */ }
                return -1;
            }

            // Detect dump
            _dumpPath = DetectNewestDump();
            return _proc.ExitCode;
        }
        finally
        {
            _proc.Dispose();
            _proc = null;
        }
    }

    public string? GetDumpOutputPath() => _dumpPath;

    private string? DetectNewestDump()
    {
        try
        {
            var dir = new DirectoryInfo(_cfg.DumpDirectory);
            if (!dir.Exists) return null;
            return dir.GetFiles("*.dmp")
                .OrderByDescending(f => f.LastWriteTimeUtc)
                .FirstOrDefault()?.FullName;
        }
        catch { return null; }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Simulation implementations — exercise same code paths with fakes
// ═══════════════════════════════════════════════════════════════

/// <summary>Fake task scheduler that logs operations but does not touch Windows Task Scheduler.</summary>
public sealed class SimulatedTaskSchedulerOps : ITaskSchedulerOps
{
    private readonly HashSet<string> _tasks = new(StringComparer.OrdinalIgnoreCase);

    public bool InstallOrUpdate(Config cfg)
    {
        string name = TaskNameHelper.Sanitize(cfg.TaskName);
        bool existed = _tasks.Contains(name);
        _tasks.Add(name);
        Logger.Log("Sim-Task", $"Task '{name}' {(existed ? "updated" : "created")} (simulated).");
        return existed;
    }

    public void StartNow(string taskName)
    {
        Logger.Log("Sim-Task", $"Task '{taskName}' started (simulated).");
    }

    public void RemoveTask(string taskName)
    {
        _tasks.Remove(taskName);
        Logger.Log("Sim-Task", $"Task '{taskName}' removed (simulated).");
    }

    public bool TaskExists(string taskName) => _tasks.Contains(taskName);

    public TaskStatusInfo GetDetailedStatus(string taskName) => new()
    {
        Exists = _tasks.Contains(taskName),
        State = _tasks.Contains(taskName) ? "Ready" : "Not installed",
        LastRunTime = "Never",
        LastRunResult = "0x0",
        NextRunTime = "N/A"
    };
}

/// <summary>
/// Creates a small fake .dmp file in the dump directory and returns immediately.
/// Exercises the same code path shape as the real runner.
/// </summary>
public sealed class SimulatedProcDumpRunner : IProcDumpRunner
{
    private string? _dumpPath;

    public void Start(Config cfg)
    {
        if (!Directory.Exists(cfg.DumpDirectory))
            Directory.CreateDirectory(cfg.DumpDirectory);

        string fakeName = $"{cfg.TargetName}_{DateTime.Now:yyyyMMdd_HHmmss}_simulated.dmp";
        _dumpPath = Path.Combine(cfg.DumpDirectory, fakeName);

        Logger.Log("Sim-ProcDump", $"Creating simulated dump: {_dumpPath}");
        File.WriteAllText(_dumpPath,
            $"SIMULATED DUMP FILE\r\n" +
            $"Target: {cfg.TargetName}\r\n" +
            $"Machine: {Environment.MachineName}\r\n" +
            $"Time: {DateTime.Now:O}\r\n" +
            $"This file was created by --simulate-dump mode.\r\n");
    }

    public int WaitForCompletion(CancellationToken ct)
    {
        // Simulate a brief processing delay
        ct.WaitHandle.WaitOne(500);
        Logger.Log("Sim-ProcDump", "Simulated ProcDump completed (exit code 0).");
        return 0;
    }

    public string? GetDumpOutputPath() => _dumpPath;
}

// ═══════════════════════════════════════════════════════════════
//  OneShot result — trim-safe JSON output
// ═══════════════════════════════════════════════════════════════

/// <summary>Structured result of a one-shot run, written to stdout as JSON.</summary>
public sealed class OneShotResult
{
    public bool Success { get; set; }
    public string TaskName { get; set; } = "";
    public bool TaskCreated { get; set; }
    public bool TaskRemoved { get; set; }
    public string? DumpFilePath { get; set; }
    public bool EmailSent { get; set; }
    public bool Simulated { get; set; }
    public string? Error { get; set; }
    public List<string> Steps { get; set; } = [];
}

// ═══════════════════════════════════════════════════════════════
//  OneShot options
// ═══════════════════════════════════════════════════════════════

/// <summary>Options parsed from CLI flags for the one-shot run.</summary>
public sealed class OneShotOptions
{
    public bool SimulateDump { get; init; }
    public bool NoEmail { get; init; }
    public bool SimulateTask { get; init; }
}

// ═══════════════════════════════════════════════════════════════
//  Orchestrator
// ═══════════════════════════════════════════════════════════════

/// <summary>
/// Executes the full one-shot sequence:
/// 1) Create/update scheduled task
/// 2) Run ProcDump (or simulate dump)
/// 3) Detect dump completion
/// 4) Send email notification
/// 5) Remove scheduled task
/// 6) Exit
///
/// All dependencies are injected — no static calls, no reflection,
/// fully trim-safe, fully testable.
/// </summary>
public sealed class OneShotRunner
{
    private readonly ITaskSchedulerOps _taskOps;
    private readonly IProcDumpRunner _procDump;
    private readonly IEmailSender _email;
    private readonly OneShotOptions _options;
    private readonly Config _cfg;
    private readonly string? _configSavePath;

    public OneShotRunner(
        Config cfg,
        ITaskSchedulerOps taskOps,
        IProcDumpRunner procDump,
        IEmailSender email,
        OneShotOptions options,
        string? configSavePath = null)
    {
        _cfg = cfg;
        _taskOps = taskOps;
        _procDump = procDump;
        _email = email;
        _options = options;
        _configSavePath = configSavePath;
    }

    /// <summary>Run the full one-shot sequence. Returns structured result.</summary>
    public OneShotResult Execute(CancellationToken ct = default)
    {
        var result = new OneShotResult
        {
            TaskName = TaskNameHelper.Sanitize(_cfg.TaskName),
            Simulated = _options.SimulateDump
        };

        try
        {
            // ── Step 1: Save config ──
            Log(result, "Saving config…");
            _cfg.Save(_configSavePath);

            // ── Step 2: Create scheduled task ──
            Log(result, $"Creating scheduled task '{result.TaskName}'…");
            bool existed = _taskOps.InstallOrUpdate(_cfg);
            result.TaskCreated = true;
            Log(result, existed
                ? $"Task '{result.TaskName}' updated."
                : $"Task '{result.TaskName}' created.");

            if (ct.IsCancellationRequested) { result.Error = "Cancelled."; return result; }

            // ── Step 3: Run ProcDump / simulate dump ──
            Log(result, _options.SimulateDump
                ? "Simulating dump creation…"
                : "Starting ProcDump…");

            _procDump.Start(_cfg);
            int exitCode = _procDump.WaitForCompletion(ct);
            string? dumpPath = _procDump.GetDumpOutputPath();
            result.DumpFilePath = dumpPath;

            Log(result, $"ProcDump exit code: {exitCode}");
            if (dumpPath != null)
                Log(result, $"Dump file: {dumpPath}");
            else
                Log(result, "No dump file detected.");

            if (ct.IsCancellationRequested) { result.Error = "Cancelled."; return result; }

            // ── Step 4: Send email ──
            if (_options.NoEmail)
            {
                Log(result, "Email skipped (--no-email flag).");
            }
            else if (!_cfg.EmailEnabled)
            {
                Log(result, "Email skipped (not enabled in config).");
            }
            else
            {
                Log(result, $"Sending notification email to {_cfg.ToAddress}…");
                try
                {
                    if (dumpPath != null)
                        _email.SendDumpNotification(_cfg, dumpPath);
                    else
                        _email.SendTestEmail(_cfg);

                    result.EmailSent = true;
                    Log(result, "Email sent successfully.");
                }
                catch (Exception emailEx)
                {
                    Log(result, $"Email failed: {emailEx.Message}");
                    Logger.Log("OneShot", $"Email error: {emailEx}");
                    // Non-fatal — continue to cleanup
                }
            }

            // ── Step 5: Remove scheduled task ──
            Log(result, $"Removing scheduled task '{result.TaskName}'…");
            try
            {
                _taskOps.RemoveTask(result.TaskName);
                result.TaskRemoved = true;
                Log(result, "Task removed.");
            }
            catch (Exception removeEx)
            {
                Log(result, $"Task removal failed: {removeEx.Message}");
                Logger.Log("OneShot", $"Remove error: {removeEx}");
            }

            result.Success = true;
            Log(result, "One-shot sequence completed successfully.");
        }
        catch (Exception ex)
        {
            result.Error = ex.Message;
            Log(result, $"FATAL: {ex.Message}");
            Logger.Log("OneShot", $"Fatal error: {ex}");

            // Best-effort cleanup: remove task if we created it
            if (result.TaskCreated && !result.TaskRemoved)
            {
                try
                {
                    _taskOps.RemoveTask(result.TaskName);
                    result.TaskRemoved = true;
                    Log(result, "Task removed (cleanup after error).");
                }
                catch { /* swallow during cleanup */ }
            }
        }

        return result;
    }

    private static void Log(OneShotResult result, string message)
    {
        string line = $"[{DateTime.Now:HH:mm:ss}] {message}";
        result.Steps.Add(line);
        Logger.Log("OneShot", message);
        Console.WriteLine(line);
    }
}

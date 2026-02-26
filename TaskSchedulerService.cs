using Microsoft.Win32.TaskScheduler;

namespace ProcDumpMonitor;

/// <summary>Detailed snapshot of scheduled task state.</summary>
public sealed class TaskStatusInfo
{
    public bool Exists { get; init; }
    public string State { get; init; } = "";
    public string LastRunTime { get; init; } = "";
    public string LastRunResult { get; init; } = "";
    public string NextRunTime { get; init; } = "";
}

/// <summary>Describes the action the scheduled task will execute.</summary>
public sealed class TaskActionPreview
{
    public string ExePath { get; init; } = "";
    public string Arguments { get; init; } = "";
    public string WorkingDirectory { get; init; } = "";

    public override string ToString()
        => $"EXE:       {ExePath}\r\nArguments: {Arguments}\r\nWork Dir:  {WorkingDirectory}";
}

/// <summary>Manages the Windows Scheduled Task for ProcDump monitoring.</summary>
public static class TaskSchedulerService
{
    /// <summary>Create or update the scheduled task. Returns true if it was an update (task already existed).</summary>
    public static bool InstallOrUpdate(Config cfg)
    {
        string exePath = GetExePath();
        string configPath = Config.DefaultConfigPath;

        using var ts = new TaskService();

        bool existed = ts.GetTask(cfg.TaskName) != null;

        // Remove existing task if present
        ts.RootFolder.DeleteTask(cfg.TaskName, false);

        var td = ts.NewTask();
        td.RegistrationInfo.Description =
            $"ProcDump Monitor – watches for {cfg.TargetName} and captures crash dumps.";

        // Principal: SYSTEM, highest privileges
        td.Principal.UserId = "SYSTEM";
        td.Principal.LogonType = TaskLogonType.ServiceAccount;
        td.Principal.RunLevel = TaskRunLevel.Highest;

        // Trigger: at startup
        td.Triggers.Add(new BootTrigger());

        // Action: run this EXE in monitor mode
        td.Actions.Add(new ExecAction(
            exePath,
            $"--monitor --config \"{configPath}\"",
            AppContext.BaseDirectory));

        // Settings
        td.Settings.AllowDemandStart = true;
        td.Settings.DisallowStartIfOnBatteries = false;
        td.Settings.StopIfGoingOnBatteries = false;
        td.Settings.RunOnlyIfIdle = false;
        td.Settings.IdleSettings.StopOnIdleEnd = false;
        td.Settings.StartWhenAvailable = true;
        td.Settings.RestartInterval = TimeSpan.FromMinutes(1);
        td.Settings.RestartCount = 999;
        td.Settings.MultipleInstances = TaskInstancesPolicy.IgnoreNew;
        td.Settings.ExecutionTimeLimit = TimeSpan.Zero; // no time limit

        ts.RootFolder.RegisterTaskDefinition(
            cfg.TaskName,
            td,
            TaskCreation.CreateOrUpdate,
            "SYSTEM",
            null,
            TaskLogonType.ServiceAccount);

        return existed;
    }

    /// <summary>Run the task now (demand start).</summary>
    public static void StartNow(string taskName)
    {
        using var ts = new TaskService();
        var task = ts.GetTask(taskName)
            ?? throw new InvalidOperationException($"Task '{taskName}' not found.");
        task.Run();
    }

    /// <summary>Stop the running task.</summary>
    public static void StopTask(string taskName)
    {
        using var ts = new TaskService();
        var task = ts.GetTask(taskName)
            ?? throw new InvalidOperationException($"Task '{taskName}' not found.");
        task.Stop();
    }

    /// <summary>Remove the task.</summary>
    public static void RemoveTask(string taskName)
    {
        using var ts = new TaskService();
        ts.RootFolder.DeleteTask(taskName, false);
    }

    /// <summary>Check whether the task currently exists.</summary>
    public static bool TaskExists(string taskName)
    {
        using var ts = new TaskService();
        return ts.GetTask(taskName) != null;
    }

    /// <summary>Get a detailed snapshot of the task state.</summary>
    public static TaskStatusInfo GetDetailedStatus(string taskName)
    {
        using var ts = new TaskService();
        var task = ts.GetTask(taskName);
        if (task == null)
            return new TaskStatusInfo { Exists = false, State = "Not installed" };

        return new TaskStatusInfo
        {
            Exists = true,
            State = task.State.ToString(),
            LastRunTime = task.LastRunTime == DateTime.MinValue ? "Never" : task.LastRunTime.ToString("g"),
            LastRunResult = $"0x{task.LastTaskResult:X}",
            NextRunTime = task.NextRunTime == DateTime.MinValue ? "N/A" : task.NextRunTime.ToString("g"),
        };
    }

    /// <summary>Get a one-line summary of the task state (legacy helper).</summary>
    public static string GetTaskStatus(string taskName)
    {
        var info = GetDetailedStatus(taskName);
        if (!info.Exists) return "Not installed";
        return $"{info.State} (last run: {info.LastRunTime}, result: {info.LastRunResult})";
    }

    /// <summary>Build a preview of the action the scheduled task will execute.</summary>
    public static TaskActionPreview BuildActionPreview(Config cfg)
    {
        return new TaskActionPreview
        {
            ExePath = GetExePath(),
            Arguments = $"--monitor --config \"{Config.DefaultConfigPath}\"",
            WorkingDirectory = AppContext.BaseDirectory
        };
    }

    private static string GetExePath()
    {
        // Works for both single-file and normal publish
        string? path = Environment.ProcessPath;
        if (!string.IsNullOrEmpty(path))
            return path;

        return Path.Combine(AppContext.BaseDirectory, "ProcDumpMonitor.exe");
    }
}

namespace ProcDumpMonitor;

/// <summary>Result of validating ProcDump option combinations.</summary>
public sealed class ProcDumpValidationResult
{
    public bool IsValid => Errors.Count == 0;
    public List<string> Errors { get; } = new();
    public List<string> Warnings { get; } = new();
}

/// <summary>
/// Validates ProcDump option combinations and required parameters.
/// Checks for conflicts, missing dependencies, and out-of-range values.
/// </summary>
public static class ProcDumpOptionsValidator
{
    public static ProcDumpValidationResult Validate(Config cfg)
    {
        var result = new ProcDumpValidationResult();

        // ── Required fields ──
        if (string.IsNullOrWhiteSpace(cfg.ProcDumpPath))
            result.Errors.Add("ProcDump executable path is required.");
        else if (!File.Exists(cfg.ProcDumpPath))
            result.Errors.Add($"ProcDump executable not found: {cfg.ProcDumpPath}");

        if (string.IsNullOrWhiteSpace(cfg.DumpDirectory))
            result.Errors.Add("Dump directory must be specified.");

        // ── Dump type ──
        var validTypes = new[] { "Full", "MiniPlus", "Mini", "ThreadDump" };
        if (!validTypes.Contains(cfg.DumpType, StringComparer.OrdinalIgnoreCase))
            result.Errors.Add($"Invalid dump type '{cfg.DumpType}'. Valid types: {string.Join(", ", validTypes)}.");

        if (cfg.DumpType == "ThreadDump")
            result.Warnings.Add("Thread dump (-mt) creates a text file, not a .dmp binary. Dump detection and stability checks may not apply.");

        // ── CPU thresholds ──
        if (cfg.CpuThreshold < 0 || cfg.CpuThreshold > 100)
            result.Errors.Add("CPU threshold must be between 0 and 100.");
        if (cfg.CpuLowThreshold < 0 || cfg.CpuLowThreshold > 100)
            result.Errors.Add("CPU low threshold must be between 0 and 100.");

        if (cfg.CpuThreshold > 0 && cfg.CpuLowThreshold > 0)
            result.Errors.Add("Cannot specify both CPU-above (-c) and CPU-below (-cl) thresholds at the same time.");

        if (cfg.CpuThreshold > 0 && cfg.CpuThreshold < 10)
            result.Warnings.Add($"CPU threshold {cfg.CpuThreshold}% is very low and may produce frequent dumps.");

        if (cfg.CpuDurationSeconds > 0 && cfg.CpuThreshold == 0 && cfg.CpuLowThreshold == 0)
            result.Errors.Add("CPU duration (-s) requires a CPU threshold (-c or -cl) to be set.");

        if (cfg.CpuPerUnit && cfg.CpuThreshold == 0 && cfg.CpuLowThreshold == 0)
            result.Errors.Add("Per-CPU flag (-u) requires a CPU threshold (-c or -cl) to be set.");

        // ── Memory ──
        if (cfg.MemoryCommitMB < 0)
            result.Errors.Add("Memory threshold must be zero or positive.");

        // ── Max dumps ──
        if (cfg.MaxDumps < 1)
            result.Errors.Add("Number of dumps must be at least 1.");

        // ── No trigger warning ──
        bool hasTrigger = cfg.DumpOnException || cfg.DumpOnTerminate ||
                          cfg.HangWindowSeconds > 0 ||
                          cfg.CpuThreshold > 0 || cfg.CpuLowThreshold > 0 ||
                          cfg.MemoryCommitMB > 0 ||
                          !string.IsNullOrWhiteSpace(cfg.PerformanceCounter) ||
                          !string.IsNullOrWhiteSpace(cfg.PerfCounterThreshold);

        if (!hasTrigger)
            result.Warnings.Add("No trigger is active — ProcDump will capture a dump immediately when the target process is found.");

        // ── Advanced ──
        if (cfg.AvoidTerminateTimeout < 0)
            result.Errors.Add("Avoid-terminate timeout must be zero or positive.");

        return result;
    }
}

namespace ProcDumpMonitor;

public static class ProcDumpPreset
{
    /// <summary>A named scenario that populates recommended ProcDump options.</summary>
    public sealed class Preset
    {
        public string Name { get; }
        public string Description { get; }
        public string EffectiveFlags { get; }
        private readonly Action<Config> _apply;

        private Preset(string name, string description, string effectiveFlags, Action<Config> apply)
        {
            Name = name;
            Description = description;
            EffectiveFlags = effectiveFlags;
            _apply = apply;
        }

        /// <summary>Reset all ProcDump-related config fields to safe defaults, then apply the preset.</summary>
        public void Apply(Config cfg)
        {
            ResetProcDumpFields(cfg);
            _apply(cfg);
        }

        public override string ToString() => Name;

        /// <summary>All built-in presets. The first entry is the recommended default ("Crash capture").
        /// Order: Crash capture → Hang → High CPU → Memory → Low impact.</summary>
        public static IReadOnlyList<Preset> All { get; } = new Preset[]
        {
            new Preset("Crash capture",
                "Captures a full dump when the process throws an unhandled exception or terminates unexpectedly. " +
                "Uses safe defaults appropriate for production systems. Ideal for post-mortem crash investigation.",
                "-ma -e -t",
                cfg =>
                {
                    cfg.DumpType = "Full";
                    cfg.DumpOnException = true;
                    cfg.DumpOnTerminate = true;
                }),

            new Preset("Hang capture",
                "Captures a full dump when the process window stops responding (hung). " +
                "Useful for diagnosing UI freezes and deadlocks.",
                "-ma -h",
                cfg =>
                {
                    cfg.DumpType = "Full";
                    cfg.HangWindowSeconds = 1; // enables -h
                }),

            new Preset("High CPU spike capture",
                "Captures up to 3 full dumps when CPU usage exceeds 90 % for at least 10 consecutive seconds. " +
                "Helps identify runaway threads or hot code paths.",
                "-ma -c 90 -s 10 -n 3",
                cfg =>
                {
                    cfg.DumpType = "Full";
                    cfg.CpuThreshold = 90;
                    cfg.CpuDurationSeconds = 10;
                    cfg.MaxDumps = 3;
                }),

            new Preset("Memory threshold capture",
                "Captures up to 3 full dumps when process memory commit exceeds 2048 MB. " +
                "Useful for investigating memory leaks or unexpected memory growth.",
                "-ma -m 2048 -n 3",
                cfg =>
                {
                    cfg.DumpType = "Full";
                    cfg.MemoryCommitMB = 2048;
                    cfg.MaxDumps = 3;
                }),

            new Preset("Low impact full dump",
                "A full memory dump equivalent to Task Manager, captured via process cloning (-r) to minimize disruption. " +
                "The -a flag prevents dump floods; the process is suspended for only milliseconds instead of the full dump duration.",
                "-a -r -ma",
                cfg =>
                {
                    cfg.DumpType = "Full";
                    cfg.AvoidOutage = true;
                    cfg.UseClone = true;
                    cfg.MaxDumps = 1;
                }),
        };

        /// <summary>Find a preset by name (case-insensitive).</summary>
        public static Preset? FindByName(string name) =>
            All.FirstOrDefault(p => p.Name.Equals(name, StringComparison.OrdinalIgnoreCase));

        /// <summary>Zero out all ProcDump trigger/operational fields before applying a preset.</summary>
        private static void ResetProcDumpFields(Config cfg)
        {
            cfg.DumpType = "Full";
            cfg.DumpOnException = false;
            cfg.DumpOnTerminate = false;
            cfg.UseClone = false;
            cfg.AvoidOutage = false;
            cfg.OverwriteExisting = false;
            cfg.CpuPerUnit = false;
            cfg.CpuThreshold = 0;
            cfg.CpuLowThreshold = 0;
            cfg.CpuDurationSeconds = 0;
            cfg.MemoryCommitMB = 0;
            cfg.HangWindowSeconds = 0;
            cfg.MaxDumps = 1;
            cfg.WerIntegration = false;
            cfg.AvoidTerminateTimeout = 0;
            cfg.PerformanceCounter = "";
            cfg.PerfCounterThreshold = "";
            cfg.ExceptionFilterInclude = "";
            cfg.ExceptionFilterExclude = "";
            // WaitForProcess left at its current value (user preference)
            // ProcDumpPath, DumpDirectory, TargetName, RestartDelay left untouched
        }
    }
}

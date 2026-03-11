namespace ProcDumpMonitor;

/// <summary>Where the option appears in the UI.</summary>
public enum OptionVisibility { Common, Advanced, InternalOnly }

/// <summary>Category grouping for the option.</summary>
public enum OptionCategory { DumpType, Trigger, Cpu, Memory, Operational, Target, PerformanceCounter, ExceptionFilter, Wer, Timeout, Specialized }

/// <summary>Describes a single ProcDump command-line switch.</summary>
public sealed record ProcDumpOption(
    string Flag,
    string Parameters,
    OptionCategory Category,
    OptionVisibility Visibility,
    string ShortDescription,
    string PlainEnglish,
    string WhenToUse);

/// <summary>
/// Complete internal catalog of every ProcDump capture switch.
/// Only <see cref="OptionVisibility.Common"/> and <see cref="OptionVisibility.Advanced"/>
/// entries are shown in the UI; <see cref="OptionVisibility.InternalOnly"/> entries exist
/// for documentation generation only.
/// </summary>
public static class ProcDumpOptionCatalog
{
    public static IReadOnlyList<ProcDumpOption> All { get; } = new ProcDumpOption[]
    {
        // ── Dump Types ──
        new("-ma", "",
            OptionCategory.DumpType, OptionVisibility.Common,
            "Full dump",
            "Captures all process memory including code, stack, heap, and mapped files. This is the largest dump type but gives the most complete picture.",
            "Use when the support team needs a complete memory image — the standard choice for crash analysis."),

        new("-mp", "",
            OptionCategory.DumpType, OptionVisibility.Common,
            "MiniPlus dump",
            "Captures thread stacks, handles, loaded modules, and all private memory pages. Smaller than a full dump but still contains enough data for most forensic analysis.",
            "Use when disk space is limited but you still need private heap data for analysis."),

        new("-mm", "",
            OptionCategory.DumpType, OptionVisibility.Common,
            "Minidump",
            "Captures only thread stacks and handle information. Very small file size but limited analysis capability.",
            "Use when you only need call stacks and basic metadata, or when network transfer size matters."),

        new("-mt", "",
            OptionCategory.DumpType, OptionVisibility.Common,
            "Thread dump (text)",
            "Writes a plain-text file with all thread call stacks instead of a binary .dmp file. No memory image is captured.",
            "Use for a quick look at what the process threads are doing without capturing a heavy dump file."),

        new("-mc", "<mask>",
            OptionCategory.DumpType, OptionVisibility.InternalOnly,
            "Custom dump mask",
            "Writes a dump using a custom combination of MINIDUMP_TYPE flags specified as a hex mask.",
            "Rarely needed; use only when directed by advanced support."),

        new("-md", "<callback_DLL>",
            OptionCategory.DumpType, OptionVisibility.InternalOnly,
            "Callback DLL dump",
            "Writes a dump by calling a MiniDumpWriteDump callback implemented in the specified DLL.",
            "Rarely needed; use only when a custom dump callback DLL has been provided."),

        // ── Triggers ──
        new("-e", "[1 [-g] [-b]]",
            OptionCategory.Trigger, OptionVisibility.Common,
            "Exception trigger",
            "Creates a dump when the process throws an unhandled exception. Optionally, adding '1' also captures first-chance exceptions.",
            "Use to capture crash dumps automatically when a process hits an unhandled exception."),

        new("-h", "",
            OptionCategory.Trigger, OptionVisibility.Common,
            "Hang trigger",
            "Creates a dump when a process window stops responding to messages for at least 5 seconds.",
            "Use to diagnose UI freezes or deadlocks in GUI applications."),

        new("-t", "",
            OptionCategory.Trigger, OptionVisibility.Common,
            "Terminate trigger",
            "Creates a dump when the process terminates.",
            "Use to capture state at the moment of process exit, even if no exception occurred (e.g., Environment.Exit or TerminateProcess)."),

        // ── CPU ──
        new("-c", "<threshold>",
            OptionCategory.Cpu, OptionVisibility.Common,
            "CPU % above threshold",
            "Creates a dump when total CPU usage of the process exceeds the specified percentage (1–100).",
            "Use to capture a snapshot during a CPU spike so the support team can identify the hot code path."),

        new("-cl", "<threshold>",
            OptionCategory.Cpu, OptionVisibility.Common,
            "CPU % below threshold",
            "Creates a dump when total CPU usage of the process drops below the specified percentage (1–100).",
            "Use to capture state when a process unexpectedly becomes idle."),

        new("-s", "<seconds>",
            OptionCategory.Cpu, OptionVisibility.Common,
            "Duration (seconds)",
            "The CPU or memory threshold must be continuously met for this many consecutive seconds before a dump is created.",
            "Use to avoid false-positive dumps from short, harmless CPU spikes. A value of 5–10 seconds filters most transient spikes."),

        new("-n", "<count>",
            OptionCategory.Cpu, OptionVisibility.Common,
            "Number of dumps",
            "The total number of dumps to capture before ProcDump exits. The monitoring loop then restarts ProcDump for the next cycle.",
            "Use 1 for a single snapshot; use 3–5 when you need multiple samples during a sustained issue."),

        new("-u", "",
            OptionCategory.Cpu, OptionVisibility.Common,
            "Per-CPU threshold",
            "Treats the CPU threshold as a percentage of a single logical processor instead of total system CPU.",
            "Use on multi-core machines when the target process is single-threaded and the CPU percentage appears low relative to total cores."),

        // ── Memory ──
        new("-m", "<commit_MB>",
            OptionCategory.Memory, OptionVisibility.Common,
            "Memory commit above (MB)",
            "Creates a dump when the process private commit charge exceeds the specified value in megabytes.",
            "Use to capture a memory leak in progress — set the threshold just above the normal working set."),

        new("-ml", "<commit_MB>",
            OptionCategory.Memory, OptionVisibility.InternalOnly,
            "Memory commit below (MB)",
            "Creates a dump when the process private commit charge drops below the specified value in megabytes.",
            "Rarely needed; use when a process unexpectedly releases memory."),

        // ── Performance Counter ──
        new("-p", "<counter\\threshold>",
            OptionCategory.PerformanceCounter, OptionVisibility.Advanced,
            "Perf counter above threshold",
            "Creates a dump when a Windows performance counter exceeds the threshold. Specify as \"\\Category\\Counter\\threshold\" format.",
            "Use for advanced monitoring, e.g., '\\Process(myapp)\\Handle Count\\10000' to dump when handles exceed 10,000."),

        new("-pl", "<counter\\threshold>",
            OptionCategory.PerformanceCounter, OptionVisibility.Advanced,
            "Perf counter below threshold",
            "Creates a dump when a Windows performance counter drops below the threshold.",
            "Use to detect when a resource drops unexpectedly, e.g., available memory falls below a minimum."),

        // ── Exception Filtering ──
        new("-f", "<filter,...>",
            OptionCategory.ExceptionFilter, OptionVisibility.Advanced,
            "Exception include filter",
            "Only dump when the exception message contains one of the specified comma-separated substrings.",
            "Use to narrow crash captures to specific exception types, e.g., 'OutOfMemory,StackOverflow'."),

        new("-fx", "<filter,...>",
            OptionCategory.ExceptionFilter, OptionVisibility.Advanced,
            "Exception exclude filter",
            "Skip the dump if the exception message contains any of the specified comma-separated substrings.",
            "Use to suppress dumps for known, benign exceptions while still capturing others."),

        // ── Operational ──
        new("-r", "[1..5]",
            OptionCategory.Operational, OptionVisibility.Common,
            "Clone / reflect",
            "Uses PssCaptureSnapshot to clone the process before writing the dump. The original process is suspended for only a few milliseconds instead of the full dump duration.",
            "Recommended for production services where minimising disruption is critical."),

        new("-a", "",
            OptionCategory.Operational, OptionVisibility.Common,
            "Avoid outage",
            "Exits ProcDump if triggers fire too rapidly in succession, preventing a flood of dumps from overwhelming the system.",
            "Recommended for production: protects against a crash loop filling the disk or degrading the service."),

        new("-o", "",
            OptionCategory.Operational, OptionVisibility.Common,
            "Overwrite existing",
            "Overwrites an existing dump file with the same name instead of creating a new one.",
            "Use when you only need the most recent dump and want to keep disk usage constant."),

        new("-w", "",
            OptionCategory.Target, OptionVisibility.Common,
            "Wait for process",
            "If the target process is not yet running, ProcDump waits for it to launch instead of exiting immediately.",
            "Always recommended when monitoring a service that may restart: ensures ProcDump attaches as soon as the process appears."),

        new("-accepteula", "",
            OptionCategory.Operational, OptionVisibility.Common,
            "Accept EULA",
            "Automatically accepts the Sysinternals End User License Agreement. Required for unattended operation (no interactive EULA dialog).",
            "Always enabled by ProcDump Monitor — required for scheduled-task and headless operation."),

        // ── WER ──
        new("-wer", "",
            OptionCategory.Wer, OptionVisibility.Advanced,
            "WER integration",
            "Registers ProcDump as the Windows Error Reporting (WER) just-in-time debugger for the target.",
            "Use when the standard exception trigger (-e) is not capturing the crash because WER handles it first."),

        // ── Timeout ──
        new("-at", "<seconds>",
            OptionCategory.Timeout, OptionVisibility.Advanced,
            "Avoid-terminate timeout",
            "Cancels a dump-in-progress if it takes longer than the specified number of seconds to write. Prevents ProcDump from blocking process termination indefinitely.",
            "Use when the monitored process has a service-control timeout and must shut down within a deadline."),

        // ── Specialized / Internal Only ──
        new("-b", "",
            OptionCategory.Specialized, OptionVisibility.InternalOnly,
            "Breakpoint as exception",
            "Treats debug breakpoints (int 3) as exceptions. Requires -e.",
            "Advanced debugging scenario only."),

        new("-d", "",
            OptionCategory.Specialized, OptionVisibility.InternalOnly,
            "Diagnostic / debug output",
            "Enables ProcDump's own diagnostic output for troubleshooting ProcDump itself.",
            "Use when ProcDump is not behaving as expected and you need its internal logs."),

        new("-g", "",
            OptionCategory.Specialized, OptionVisibility.InternalOnly,
            "Native debugger",
            "Runs ProcDump as a native debugger instead of using non-invasive process snapshotting.",
            "Advanced: required for some first-chance exception scenarios."),

        new("-i", "",
            OptionCategory.Specialized, OptionVisibility.InternalOnly,
            "Install as AeDebug",
            "Installs ProcDump as the system post-mortem (AeDebug) debugger.",
            "Machine-wide setting; use with caution."),

        new("-k", "",
            OptionCategory.Specialized, OptionVisibility.InternalOnly,
            "Kill after dump",
            "Terminates the target process after the dump is captured.",
            "Use when you need a dump and also want to force-restart the process."),

        new("-l", "",
            OptionCategory.Specialized, OptionVisibility.InternalOnly,
            "Debug logging",
            "Displays low-level debug logging from the target process.",
            "Internal debugging only."),
    };

    /// <summary>Options shown in the default (common) options UI area.</summary>
    public static IReadOnlyList<ProcDumpOption> Common { get; } =
        All.Where(o => o.Visibility == OptionVisibility.Common).ToList();

    /// <summary>Options shown behind the "Advanced" toggle.</summary>
    public static IReadOnlyList<ProcDumpOption> Advanced { get; } =
        All.Where(o => o.Visibility == OptionVisibility.Advanced).ToList();

    /// <summary>Look up an option by its flag string (e.g. "-ma").</summary>
    public static ProcDumpOption? FindByFlag(string flag) =>
        All.FirstOrDefault(o => o.Flag.Equals(flag, StringComparison.OrdinalIgnoreCase));
}

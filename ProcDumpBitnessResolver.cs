using System.Diagnostics;
using System.Runtime.InteropServices;

namespace ProcDumpMonitor;

/// <summary>Detected bitness of the target process.</summary>
public enum TargetBitness { Unknown, X86, X64 }

/// <summary>Result of resolving which ProcDump binary to use.</summary>
public sealed class BitnessResult
{
    public TargetBitness Bitness { get; init; }
    public string RecommendedBinary { get; init; } = "";
    public string ActualBinary { get; init; } = "";
    public string? Warning { get; init; }
    public string Summary { get; init; } = "";
}

/// <summary>
/// Detects whether the target process is 32-bit or 64-bit and selects
/// the appropriate ProcDump binary (procdump.exe vs procdump64.exe).
///
/// Uses IsWow64Process2 (Windows 10 1709+ / build 16299+) when available;
/// falls back to IsWow64Process on older systems.
/// Both APIs require only PROCESS_QUERY_LIMITED_INFORMATION access.
/// </summary>
public static class ProcDumpBitnessResolver
{
    private const int PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    private const ushort IMAGE_FILE_MACHINE_I386 = 0x014C;
    private const ushort IMAGE_FILE_MACHINE_AMD64 = 0x8664;
    private const ushort IMAGE_FILE_MACHINE_ARM64 = 0xAA64;

    // ── P/Invoke ──

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(int dwDesiredAccess, bool bInheritHandle, int dwProcessId);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CloseHandle(IntPtr hObject);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool IsWow64Process(IntPtr hProcess, [MarshalAs(UnmanagedType.Bool)] out bool wow64Process);

    // IsWow64Process2 is only available on Windows 10 1709+
    [DllImport("kernel32.dll", SetLastError = true, EntryPoint = "IsWow64Process2")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool IsWow64Process2Native(IntPtr hProcess, out ushort pProcessMachine, out ushort pNativeMachine);

    private static bool _isWow64Process2Available = true; // assume yes, set false on first failure

    /// <summary>
    /// Resolve the correct ProcDump binary for the given target process.
    /// Looks in <paramref name="procDumpDir"/> for procdump64.exe and procdump.exe.
    /// </summary>
    public static BitnessResult Resolve(string processName, string procDumpDir)
    {
        var bitness = DetectProcessBitness(processName);
        return SelectBinary(bitness, procDumpDir);
    }

    /// <summary>
    /// Given a known bitness, select the appropriate ProcDump binary from the directory.
    /// This method is deterministic and testable without P/Invoke.
    /// </summary>
    internal static BitnessResult SelectBinary(TargetBitness bitness, string procDumpDir)
    {
        string pd64 = Path.Combine(procDumpDir, "procdump64.exe");
        string pd32 = Path.Combine(procDumpDir, "procdump.exe");
        bool has64 = File.Exists(pd64);
        bool has32 = File.Exists(pd32);

        if (!has64 && !has32)
        {
            return new BitnessResult
            {
                Bitness = bitness,
                RecommendedBinary = "",
                ActualBinary = "",
                Warning = "Neither procdump.exe nor procdump64.exe found in the ProcDump directory.",
                Summary = "No ProcDump binary found"
            };
        }

        // On a 32-bit OS, only procdump.exe works
        if (!Environment.Is64BitOperatingSystem)
        {
            string binary = has32 ? pd32 : pd64;
            string? warn = has32 ? null : "procdump.exe not found; using procdump64.exe but it may not work on a 32-bit OS.";
            return new BitnessResult
            {
                Bitness = TargetBitness.X86,
                RecommendedBinary = pd32,
                ActualBinary = binary,
                Warning = warn,
                Summary = "32-bit OS → procdump.exe"
            };
        }

        // 64-bit OS — select based on target bitness
        string recommended;
        string actual;
        string? warning = null;
        string summary;

        switch (bitness)
        {
            case TargetBitness.X86:
                recommended = pd32;
                if (has32)
                {
                    actual = pd32;
                    summary = "32-bit process → procdump.exe";
                }
                else
                {
                    actual = pd64;
                    warning = "procdump.exe not found — falling back to procdump64.exe. " +
                              "This may work but procdump.exe is preferred for 32-bit targets.";
                    summary = "32-bit process → procdump64.exe (fallback)";
                }
                break;

            case TargetBitness.X64:
                recommended = pd64;
                if (has64)
                {
                    actual = pd64;
                    summary = "64-bit process → procdump64.exe";
                }
                else
                {
                    actual = pd32;
                    warning = "procdump64.exe not found — falling back to procdump.exe. " +
                              "This may work but procdump64.exe is preferred for 64-bit targets.";
                    summary = "64-bit process → procdump.exe (fallback)";
                }
                break;

            default: // Unknown
                // Default to procdump64.exe on 64-bit OS
                recommended = pd64;
                actual = has64 ? pd64 : pd32;
                summary = has64
                    ? "Unknown bitness → procdump64.exe (default for 64-bit OS)"
                    : "Unknown bitness → procdump.exe (procdump64.exe not found)";
                if (!has64)
                    warning = "procdump64.exe not found; using procdump.exe as fallback.";
                break;
        }

        return new BitnessResult
        {
            Bitness = bitness,
            RecommendedBinary = recommended,
            ActualBinary = actual,
            Warning = warning,
            Summary = summary
        };
    }

    /// <summary>
    /// Detect the bitness of a process by name.
    /// Returns <see cref="TargetBitness.Unknown"/> if the process is not running or detection fails.
    /// </summary>
    public static TargetBitness DetectProcessBitness(string processName)
    {
        if (!Environment.Is64BitOperatingSystem)
            return TargetBitness.X86; // 32-bit OS: everything is 32-bit

        // Strip .exe if the caller included it
        string name = processName.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
            ? processName[..^4]
            : processName;

        Process[] procs;
        try
        {
            procs = Process.GetProcessesByName(name);
        }
        catch
        {
            return TargetBitness.Unknown;
        }

        if (procs.Length == 0)
            return TargetBitness.Unknown;

        try
        {
            return DetectProcessBitness(procs[0].Id);
        }
        catch
        {
            return TargetBitness.Unknown;
        }
        finally
        {
            foreach (var p in procs) p.Dispose();
        }
    }

    /// <summary>
    /// Detect the bitness of a specific process by PID.
    /// Requires PROCESS_QUERY_LIMITED_INFORMATION access on the target.
    /// </summary>
    internal static TargetBitness DetectProcessBitness(int processId)
    {
        IntPtr hProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, processId);
        if (hProcess == IntPtr.Zero)
        {
            Logger.Log("Bitness", $"Cannot open PID {processId} (error {Marshal.GetLastWin32Error()}). Bitness unknown.");
            return TargetBitness.Unknown;
        }

        try
        {
            // Try IsWow64Process2 first (Windows 10 1709+)
            if (_isWow64Process2Available)
            {
                try
                {
                    if (IsWow64Process2Native(hProcess, out ushort processMachine, out ushort nativeMachine))
                    {
                        Logger.Log("Bitness", $"IsWow64Process2: processMachine=0x{processMachine:X4}, nativeMachine=0x{nativeMachine:X4}");

                        if (processMachine == IMAGE_FILE_MACHINE_I386)
                            return TargetBitness.X86;

                        // processMachine == 0 (IMAGE_FILE_MACHINE_UNKNOWN) means native arch
                        if (nativeMachine == IMAGE_FILE_MACHINE_AMD64 || nativeMachine == IMAGE_FILE_MACHINE_ARM64)
                            return TargetBitness.X64;

                        return TargetBitness.X86;
                    }
                }
                catch (EntryPointNotFoundException)
                {
                    // IsWow64Process2 not available on this OS version
                    _isWow64Process2Available = false;
                    Logger.Log("Bitness", "IsWow64Process2 not available — falling back to IsWow64Process.");
                }
            }

            // Fallback: IsWow64Process (available on all 64-bit Windows)
            if (IsWow64Process(hProcess, out bool isWow64))
            {
                Logger.Log("Bitness", $"IsWow64Process: isWow64={isWow64}");
                return isWow64 ? TargetBitness.X86 : TargetBitness.X64;
            }

            return TargetBitness.Unknown;
        }
        finally
        {
            CloseHandle(hProcess);
        }
    }
}

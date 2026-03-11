using System.Diagnostics;
using System.Security.Principal;

namespace ProcDumpMonitor;

/// <summary>
/// Centralised elevation (UAC) helpers used by Program.Main and CLI verbs.
/// </summary>
internal static class ElevationHelper
{
    /// <summary>Returns true when the current process is running as Administrator.</summary>
    public static bool IsElevated()
    {
        try
        {
            using var identity = WindowsIdentity.GetCurrent();
            var principal = new WindowsPrincipal(identity);
            return principal.IsInRole(WindowsBuiltInRole.Administrator);
        }
        catch
        {
            return false;
        }
    }

    /// <summary>
    /// Relaunch the current executable elevated (UAC prompt) with the given arguments.
    /// Returns <c>true</c> if the elevated process was started successfully,
    /// or <c>false</c> if the user cancelled the UAC prompt.
    /// </summary>
    public static bool RelaunchElevated(string[] args)
    {
        string exePath = Environment.ProcessPath
            ?? throw new InvalidOperationException("Cannot determine the current executable path.");

        var psi = new ProcessStartInfo
        {
            FileName = exePath,
            Arguments = QuoteArgs(args),
            UseShellExecute = true,
            Verb = "runas"
        };

        try
        {
            Process.Start(psi);
            return true;
        }
        catch (System.ComponentModel.Win32Exception)
        {
            // User cancelled the UAC dialog
            return false;
        }
    }

    /// <summary>
    /// Build a single command-line string from an argument array,
    /// quoting any value that contains spaces.
    /// </summary>
    private static string QuoteArgs(string[] args)
    {
        if (args.Length == 0)
            return string.Empty;

        var parts = new string[args.Length];
        for (int i = 0; i < args.Length; i++)
        {
            parts[i] = args[i].Contains(' ') ? $"\"{args[i]}\"" : args[i];
        }
        return string.Join(' ', parts);
    }
}

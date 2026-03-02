using System.Reflection;

namespace ProcDumpMonitor;

public static class BuildInfo
{
    public static string BuildDate { get; } = typeof(BuildInfo).Assembly
        .GetCustomAttributes<AssemblyMetadataAttribute>()
        .FirstOrDefault(a => a.Key == "BuildDate")?.Value ?? "dev";
}

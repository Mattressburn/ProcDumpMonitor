using System.Diagnostics;
using System.IO.Compression;
using System.Net.NetworkInformation;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace ProcDumpMonitor;

/// <summary>JSON model for the bundle manifest.</summary>
public sealed class BundleManifest
{
    public string GeneratedUtc { get; set; } = "";
    public string HostName { get; set; } = "";
    public string AppVersion { get; set; } = "";
    public List<BundleManifestEntry> Entries { get; set; } = new();
    public List<string> Errors { get; set; } = new();
}

public sealed class BundleManifestEntry
{
    public string Source { get; set; } = "";
    public string ArchivePath { get; set; } = "";
    public long BytesCopied { get; set; }
    public string? Error { get; set; }
}

/// <summary>JSON model for system_info.json.</summary>
public sealed class SystemInfoData
{
    public string HostName { get; set; } = "";
    public string BootTime { get; set; } = "";
    public string Cpu { get; set; } = "";
    public string OsVersion { get; set; } = "";
    public string SystemType { get; set; } = "";
    public string UserName { get; set; } = "";
    public string LogonDomain { get; set; } = "";
    public string LogonServer { get; set; } = "";
    public string MachineDomain { get; set; } = "";
    public string TotalMemoryMB { get; set; } = "";
    public string AvailableMemoryMB { get; set; } = "";
    public List<DiskSpaceEntry> FreeSpace { get; set; } = new();
    public string DefaultGateway { get; set; } = "";
    public string DhcpServer { get; set; } = "";
    public string DnsServer { get; set; } = "";
    public string IpSummary { get; set; } = "";
    public List<string> IpAddresses { get; set; } = new();
    public List<string> MacAddresses { get; set; } = new();
    public string SubnetMask { get; set; } = "";
    public string CcureVersion { get; set; } = "";
}

public sealed class DiskSpaceEntry
{
    public string Drive { get; set; } = "";
    public string FreeMB { get; set; } = "";
    public string TotalMB { get; set; } = "";
}

/// <summary>Progress callback for UI consumers.</summary>
public delegate void DiagnosticsProgress(string message);

/// <summary>
/// Collects support diagnostics into a single ZIP bundle.
/// Usable from CLI, GUI, or tray — all code paths converge here.
/// Never throws on missing data; always continues collecting remaining sections.
/// </summary>
public static class SupportDiagnosticsService
{
    private const long CrossFireCapBytes = 500L * 1024 * 1024; // 500 MB

    /// <summary>Check whether the current process is running elevated.</summary>
    public static bool IsElevated() => ElevationHelper.IsElevated();

    /// <summary>
    /// Relaunch the current process elevated with --support-diagnostics and optional time range args.
    /// Returns true if the process was started (caller should exit). False if UAC was cancelled.
    /// </summary>
    public static bool RelaunchElevatedForDiagnostics(DateTime? since = null, DateTime? until = null)
    {
        try
        {
            var args = new StringBuilder("--support-diagnostics");
            if (since.HasValue)
                args.Append($" --since \"{since.Value:o}\"");
            if (until.HasValue)
                args.Append($" --until \"{until.Value:o}\"");

            var psi = new ProcessStartInfo
            {
                FileName = Environment.ProcessPath ?? "ProcDumpMonitor.exe",
                Arguments = args.ToString(),
                UseShellExecute = true,
                Verb = "runas"
            };
            Process.Start(psi);
            return true;
        }
        catch (System.ComponentModel.Win32Exception)
        {
            // User cancelled UAC
            return false;
        }
    }

    /// <summary>
    /// Create a support diagnostics bundle.
    /// </summary>
    /// <param name="since">Start of time range (UTC). Default: 24 hours ago.</param>
    /// <param name="until">End of time range (UTC). Default: now.</param>
    /// <param name="progress">Optional progress callback.</param>
    /// <returns>Full path to the created ZIP file.</returns>
    public static string CreateBundle(DateTime? since = null, DateTime? until = null, DiagnosticsProgress? progress = null)
    {
        var sinceUtc = since?.ToUniversalTime() ?? DateTime.UtcNow.AddHours(-24);
        var untilUtc = until?.ToUniversalTime() ?? DateTime.UtcNow;
        var manifest = new BundleManifest
        {
            GeneratedUtc = DateTime.UtcNow.ToString("o"),
            HostName = Environment.MachineName,
            AppVersion = typeof(SupportDiagnosticsService).Assembly.GetName().Version?.ToString() ?? "1.0.0"
        };

        string outputDir = ResolveOutputDirectory();
        string zipName = $"ProcDumpMonitor_SupportBundle_{Environment.MachineName}_{DateTime.Now:yyyyMMdd_HHmmss}.zip";
        string zipPath = Path.Combine(outputDir, zipName);

        // Work in a temp directory, then zip
        string tempDir = Path.Combine(Path.GetTempPath(), $"ProcDumpMonitor_Diag_{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);

        try
        {
            // E1: ProcDump Monitor artifacts
            progress?.Invoke("Collecting ProcDump Monitor artifacts…");
            CollectMonitorArtifacts(tempDir, manifest);

            // E2: System diagnostics
            progress?.Invoke("Collecting system information…");
            CollectSystemInfo(tempDir, manifest);

            // E3: Event Viewer logs
            progress?.Invoke("Collecting Event Viewer logs…");
            CollectEventLogs(tempDir, sinceUtc, untilUtc, manifest);

            // E4: CrossFire logs
            progress?.Invoke("Collecting CrossFire logs…");
            CollectCrossFireLogs(tempDir, sinceUtc, untilUtc, manifest);

            // E5: Manifest
            progress?.Invoke("Writing manifest…");
            WriteManifest(tempDir, manifest);

            // Create ZIP
            progress?.Invoke("Creating ZIP archive…");
            if (File.Exists(zipPath))
                File.Delete(zipPath);
            ZipFile.CreateFromDirectory(tempDir, zipPath, CompressionLevel.Optimal, includeBaseDirectory: false);

            progress?.Invoke($"Bundle created: {zipPath}");
            Logger.Log("Diagnostics", $"Support bundle created: {zipPath}");
            return zipPath;
        }
        finally
        {
            // Clean up temp directory
            try { Directory.Delete(tempDir, recursive: true); }
            catch { /* best effort */ }
        }
    }

    private static string ResolveOutputDirectory()
    {
        // 1. Try Config.DumpDirectory if set and valid
        try
        {
            var cfg = Config.Load();
            if (!string.IsNullOrWhiteSpace(cfg.DumpDirectory) && Directory.Exists(cfg.DumpDirectory))
                return cfg.DumpDirectory;
        }
        catch { /* ignore */ }

        // 2. Try AppPaths.InstallDir
        try
        {
            string installDir = AppPaths.InstallDir;
            if (Directory.Exists(installDir))
                return installDir;
        }
        catch { /* ignore */ }

        // 3. Fallback to temp
        return Path.GetTempPath();
    }

    // ─────────────────────────────────────────────────────────────
    //  E1: ProcDump Monitor artifacts
    // ─────────────────────────────────────────────────────────────

    private static void CollectMonitorArtifacts(string tempDir, BundleManifest manifest)
    {
        // Logs directory
        try
        {
            string logDir = AppPaths.LogDir;
            if (Directory.Exists(logDir))
            {
                string destLogDir = Path.Combine(tempDir, "Logs");
                Directory.CreateDirectory(destLogDir);
                long totalBytes = 0;
                foreach (var file in Directory.GetFiles(logDir))
                {
                    string destFile = Path.Combine(destLogDir, Path.GetFileName(file));
                    File.Copy(file, destFile, overwrite: true);
                    totalBytes += new FileInfo(destFile).Length;
                }
                manifest.Entries.Add(new BundleManifestEntry
                {
                    Source = "ProcDump Monitor Logs",
                    ArchivePath = "Logs/",
                    BytesCopied = totalBytes
                });
            }
        }
        catch (Exception ex)
        {
            manifest.Errors.Add($"Logs collection failed: {ex.Message}");
        }

        // health.json
        try
        {
            string healthPath = AppPaths.HealthPath;
            if (File.Exists(healthPath))
            {
                string dest = Path.Combine(tempDir, "health.json");
                File.Copy(healthPath, dest, overwrite: true);
                manifest.Entries.Add(new BundleManifestEntry
                {
                    Source = "health.json",
                    ArchivePath = "health.json",
                    BytesCopied = new FileInfo(dest).Length
                });
            }
        }
        catch (Exception ex)
        {
            manifest.Errors.Add($"health.json collection failed: {ex.Message}");
        }

        // Redacted config export
        try
        {
            var cfg = Config.Load();
            string dest = Path.Combine(tempDir, "config_redacted.json");
            ConfigExportImport.Export(cfg, dest);
            manifest.Entries.Add(new BundleManifestEntry
            {
                Source = "Config (redacted)",
                ArchivePath = "config_redacted.json",
                BytesCopied = new FileInfo(dest).Length
            });
        }
        catch (Exception ex)
        {
            manifest.Errors.Add($"Config export failed: {ex.Message}");
        }

        // --status output as status.json
        try
        {
            var cfg = Config.Load();
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
            string json = JsonSerializer.Serialize(output, AppJsonContext.Default.CliStatusOutput);
            string dest = Path.Combine(tempDir, "status.json");
            File.WriteAllText(dest, json);
            manifest.Entries.Add(new BundleManifestEntry
            {
                Source = "Task status",
                ArchivePath = "status.json",
                BytesCopied = new FileInfo(dest).Length
            });
        }
        catch (Exception ex)
        {
            manifest.Errors.Add($"Status collection failed: {ex.Message}");
        }
    }

    // ─────────────────────────────────────────────────────────────
    //  E2: System diagnostics
    // ─────────────────────────────────────────────────────────────

    private static void CollectSystemInfo(string tempDir, BundleManifest manifest)
    {
        try
        {
            var info = GatherSystemInfo();

            // JSON
            string jsonDest = Path.Combine(tempDir, "system_info.json");
            string json = JsonSerializer.Serialize(info, DiagJsonContext.Default.SystemInfoData);
            File.WriteAllText(jsonDest, json);

            // Text
            string txtDest = Path.Combine(tempDir, "system_info.txt");
            File.WriteAllText(txtDest, FormatSystemInfoText(info));

            long bytes = new FileInfo(jsonDest).Length + new FileInfo(txtDest).Length;
            manifest.Entries.Add(new BundleManifestEntry
            {
                Source = "System info",
                ArchivePath = "system_info.json + system_info.txt",
                BytesCopied = bytes
            });
        }
        catch (Exception ex)
        {
            manifest.Errors.Add($"System info collection failed: {ex.Message}");
        }
    }

    private static SystemInfoData GatherSystemInfo()
    {
        var data = new SystemInfoData
        {
            HostName = SafeGet(() => Environment.MachineName),
            OsVersion = SafeGet(() => $"{RuntimeInformation.OSDescription} ({RuntimeInformation.OSArchitecture})"),
            SystemType = SafeGet(() => $"{RuntimeInformation.OSDescription} / Process: {RuntimeInformation.ProcessArchitecture}"),
            UserName = SafeGet(() => Environment.UserName),
            LogonDomain = SafeGet(() => Environment.UserDomainName),
            MachineDomain = SafeGet(() => System.Net.NetworkInformation.IPGlobalProperties.GetIPGlobalProperties().DomainName),
        };

        // CPU
        data.Cpu = SafeGet(() =>
        {
            string? cpu = Environment.GetEnvironmentVariable("PROCESSOR_IDENTIFIER");
            int count = Environment.ProcessorCount;
            return !string.IsNullOrWhiteSpace(cpu)
                ? $"{cpu} ({count} logical processors)"
                : $"{count} logical processors";
        });

        // Boot time
        data.BootTime = SafeGet(() =>
        {
            var bootTime = DateTime.UtcNow.AddMilliseconds(-Environment.TickCount64);
            return bootTime.ToLocalTime().ToString("yyyy-MM-dd HH:mm:ss");
        });

        // Memory via performance counters or GC (best-effort)
        data.TotalMemoryMB = SafeGet(() =>
        {
            if (GetPhysicalMemoryMB(out long totalMB, out long availMB))
            {
                data.AvailableMemoryMB = availMB.ToString();
                return totalMB.ToString();
            }
            return "N/A";
        });
        if (string.IsNullOrEmpty(data.AvailableMemoryMB))
            data.AvailableMemoryMB = "N/A";

        // Logon server
        data.LogonServer = SafeGet(() => Environment.GetEnvironmentVariable("LOGONSERVER") ?? "N/A");

        // Free space (all fixed drives)
        try
        {
            foreach (var drive in DriveInfo.GetDrives())
            {
                if (drive.DriveType == DriveType.Fixed && drive.IsReady)
                {
                    data.FreeSpace.Add(new DiskSpaceEntry
                    {
                        Drive = drive.Name,
                        FreeMB = (drive.AvailableFreeSpace / (1024 * 1024)).ToString(),
                        TotalMB = (drive.TotalSize / (1024 * 1024)).ToString()
                    });
                }
            }
        }
        catch { /* best effort */ }

        // Network information
        try
        {
            var interfaces = NetworkInterface.GetAllNetworkInterfaces()
                .Where(n => n.OperationalStatus == OperationalStatus.Up
                         && n.NetworkInterfaceType != NetworkInterfaceType.Loopback)
                .ToArray();

            foreach (var nic in interfaces)
            {
                var mac = nic.GetPhysicalAddress().ToString();
                if (!string.IsNullOrWhiteSpace(mac) && mac != "000000000000")
                {
                    string formatted = string.Join(":", Enumerable.Range(0, mac.Length / 2).Select(i => mac.Substring(i * 2, 2)));
                    if (!data.MacAddresses.Contains(formatted))
                        data.MacAddresses.Add(formatted);
                }

                var ipProps = nic.GetIPProperties();

                foreach (var addr in ipProps.UnicastAddresses)
                {
                    string ip = addr.Address.ToString();
                    if (!data.IpAddresses.Contains(ip))
                        data.IpAddresses.Add(ip);

                    if (addr.Address.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork
                        && string.IsNullOrEmpty(data.SubnetMask))
                    {
                        data.SubnetMask = addr.IPv4Mask?.ToString() ?? "N/A";
                    }
                }

                foreach (var gw in ipProps.GatewayAddresses)
                {
                    if (string.IsNullOrEmpty(data.DefaultGateway))
                        data.DefaultGateway = gw.Address.ToString();
                }

                foreach (var dns in ipProps.DnsAddresses)
                {
                    if (string.IsNullOrEmpty(data.DnsServer))
                        data.DnsServer = dns.ToString();
                }

                var dhcp = ipProps.DhcpServerAddresses;
                if (dhcp.Count > 0 && string.IsNullOrEmpty(data.DhcpServer))
                    data.DhcpServer = dhcp[0].ToString();
            }

            data.IpSummary = data.IpAddresses.Count > 0
                ? string.Join(", ", data.IpAddresses.Where(a => a.Contains('.')).Take(3))
                : "N/A";
        }
        catch { /* best effort */ }

        // CCURE version (registry)
        data.CcureVersion = SafeGet(() =>
        {
            string[] regPaths =
            [
                @"SOFTWARE\Tyco\CCURE 9000",
                @"SOFTWARE\WOW6432Node\Tyco\CCURE 9000",
                @"SOFTWARE\JCI\CCURE 9000",
                @"SOFTWARE\WOW6432Node\JCI\CCURE 9000"
            ];
            foreach (var path in regPaths)
            {
                try
                {
                    using var key = Microsoft.Win32.Registry.LocalMachine.OpenSubKey(path);
                    var val = key?.GetValue("Version") ?? key?.GetValue("ProductVersion");
                    if (val != null)
                        return val.ToString() ?? "N/A";
                }
                catch { /* try next */ }
            }
            return "N/A";
        });

        // Fill defaults for anything still empty
        if (string.IsNullOrEmpty(data.DefaultGateway)) data.DefaultGateway = "N/A";
        if (string.IsNullOrEmpty(data.DhcpServer)) data.DhcpServer = "N/A";
        if (string.IsNullOrEmpty(data.DnsServer)) data.DnsServer = "N/A";
        if (string.IsNullOrEmpty(data.SubnetMask)) data.SubnetMask = "N/A";
        if (string.IsNullOrEmpty(data.IpSummary)) data.IpSummary = "N/A";
        if (string.IsNullOrEmpty(data.LogonServer)) data.LogonServer = "N/A";
        if (string.IsNullOrEmpty(data.MachineDomain)) data.MachineDomain = "N/A";

        return data;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GlobalMemoryStatusEx(ref MEMORYSTATUSEX lpBuffer);

    [StructLayout(LayoutKind.Sequential)]
    private struct MEMORYSTATUSEX
    {
        public uint dwLength;
        public uint dwMemoryLoad;
        public ulong ullTotalPhys;
        public ulong ullAvailPhys;
        public ulong ullTotalPageFile;
        public ulong ullAvailPageFile;
        public ulong ullTotalVirtual;
        public ulong ullAvailVirtual;
        public ulong ullAvailExtendedVirtual;
    }

    private static bool GetPhysicalMemoryMB(out long totalMB, out long availMB)
    {
        totalMB = 0;
        availMB = 0;
        try
        {
            var mem = new MEMORYSTATUSEX { dwLength = (uint)Marshal.SizeOf<MEMORYSTATUSEX>() };
            if (GlobalMemoryStatusEx(ref mem))
            {
                totalMB = (long)(mem.ullTotalPhys / (1024UL * 1024UL));
                availMB = (long)(mem.ullAvailPhys / (1024UL * 1024UL));
                return true;
            }
        }
        catch { /* best effort */ }
        return false;
    }

    private static string FormatSystemInfoText(SystemInfoData d)
    {
        var sb = new StringBuilder();
        sb.AppendLine("═══════════════════════════════════════════");
        sb.AppendLine("  ProcDump Monitor — System Information");
        sb.AppendLine("═══════════════════════════════════════════");
        sb.AppendLine();
        sb.AppendLine($"  Host Name:          {d.HostName}");
        sb.AppendLine($"  OS Version:         {d.OsVersion}");
        sb.AppendLine($"  System Type:        {d.SystemType}");
        sb.AppendLine($"  CPU:                {d.Cpu}");
        sb.AppendLine($"  Boot Time:          {d.BootTime}");
        sb.AppendLine($"  Memory (Total):     {d.TotalMemoryMB} MB");
        sb.AppendLine($"  Memory (Available): {d.AvailableMemoryMB} MB");
        sb.AppendLine($"  User Name:          {d.UserName}");
        sb.AppendLine($"  Logon Domain:       {d.LogonDomain}");
        sb.AppendLine($"  Logon Server:       {d.LogonServer}");
        sb.AppendLine($"  Machine Domain:     {d.MachineDomain}");
        sb.AppendLine($"  CCURE Version:      {d.CcureVersion}");
        sb.AppendLine();

        sb.AppendLine("── Disk Space ──");
        foreach (var ds in d.FreeSpace)
            sb.AppendLine($"  {ds.Drive,-10} Free: {ds.FreeMB,10} MB / {ds.TotalMB,10} MB");
        sb.AppendLine();

        sb.AppendLine("── Network ──");
        sb.AppendLine($"  IP (summary):       {d.IpSummary}");
        sb.AppendLine($"  Default Gateway:    {d.DefaultGateway}");
        sb.AppendLine($"  DHCP Server:        {d.DhcpServer}");
        sb.AppendLine($"  DNS Server:         {d.DnsServer}");
        sb.AppendLine($"  Subnet Mask:        {d.SubnetMask}");
        sb.AppendLine();
        sb.AppendLine("  IP Addresses:");
        foreach (var ip in d.IpAddresses)
            sb.AppendLine($"    {ip}");
        sb.AppendLine();
        sb.AppendLine("  MAC Addresses:");
        foreach (var mac in d.MacAddresses)
            sb.AppendLine($"    {mac}");

        return sb.ToString();
    }

    // ─────────────────────────────────────────────────────────────
    //  E3: Event Viewer logs
    // ─────────────────────────────────────────────────────────────

    private static void CollectEventLogs(string tempDir, DateTime sinceUtc, DateTime untilUtc, BundleManifest manifest)
    {
        string evtDir = Path.Combine(tempDir, "EventLogs");
        Directory.CreateDirectory(evtDir);

        // Export Application and System logs via wevtutil
        ExportEventLog("Application", evtDir, sinceUtc, untilUtc, manifest);
        ExportEventLog("System", evtDir, sinceUtc, untilUtc, manifest);

        // Generate event_summary.txt
        try
        {
            string summaryPath = Path.Combine(evtDir, "event_summary.txt");
            var sb = new StringBuilder();
            sb.AppendLine($"Event Summary (range: {sinceUtc:o} to {untilUtc:o})");
            sb.AppendLine(new string('─', 60));

            foreach (string logName in new[] { "Application", "System" })
            {
                sb.AppendLine();
                sb.AppendLine($"── {logName} Log ──");
                try
                {
                    GenerateEventSummary(logName, sinceUtc, untilUtc, sb);
                }
                catch (Exception ex)
                {
                    sb.AppendLine($"  Error reading {logName} log: {ex.Message}");
                }
            }

            File.WriteAllText(summaryPath, sb.ToString());
            manifest.Entries.Add(new BundleManifestEntry
            {
                Source = "Event summary",
                ArchivePath = "EventLogs/event_summary.txt",
                BytesCopied = new FileInfo(summaryPath).Length
            });
        }
        catch (Exception ex)
        {
            manifest.Errors.Add($"Event summary failed: {ex.Message}");
        }
    }

    private static void ExportEventLog(string logName, string destDir, DateTime sinceUtc, DateTime untilUtc, BundleManifest manifest)
    {
        string destFile = Path.Combine(destDir, $"{logName}.evtx");
        try
        {
            // Build XPath time query for wevtutil
            string sinceStr = sinceUtc.ToString("o");
            string untilStr = untilUtc.ToString("o");
            string query = $"*[System[TimeCreated[@SystemTime>='{sinceStr}' and @SystemTime<='{untilStr}']]]";

            var psi = new ProcessStartInfo
            {
                FileName = "wevtutil.exe",
                Arguments = $"epl \"{logName}\" \"{destFile}\" /q:\"{query}\" /ow:true",
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardError = true
            };

            using var proc = Process.Start(psi);
            if (proc != null)
            {
                string stderr = proc.StandardError.ReadToEnd();
                proc.WaitForExit(30_000);
                if (proc.ExitCode != 0 && !string.IsNullOrWhiteSpace(stderr))
                {
                    manifest.Errors.Add($"wevtutil {logName}: {stderr.Trim()}");
                }
            }

            if (File.Exists(destFile))
            {
                manifest.Entries.Add(new BundleManifestEntry
                {
                    Source = $"Event log: {logName}",
                    ArchivePath = $"EventLogs/{logName}.evtx",
                    BytesCopied = new FileInfo(destFile).Length
                });
            }
        }
        catch (Exception ex)
        {
            manifest.Errors.Add($"Event log export ({logName}) failed: {ex.Message}");
        }
    }

    private static void GenerateEventSummary(string logName, DateTime sinceUtc, DateTime untilUtc, StringBuilder sb)
    {
        // Use wevtutil to query events in XML and parse manually (trim-safe, no EventLog class dependency)
        int errorCount = 0;
        int warningCount = 0;
        var recentErrors = new List<string>();

        string sinceStr = sinceUtc.ToString("o");
        string untilStr = untilUtc.ToString("o");

        // Query for errors and warnings
        string query = $"*[System[TimeCreated[@SystemTime>='{sinceStr}' and @SystemTime<='{untilStr}'] and (Level=1 or Level=2 or Level=3)]]";

        var psi = new ProcessStartInfo
        {
            FileName = "wevtutil.exe",
            Arguments = $"qe \"{logName}\" /q:\"{query}\" /f:text /rd:true /c:500",
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };

        using var proc = Process.Start(psi);
        if (proc == null) return;

        string output = proc.StandardOutput.ReadToEnd();
        proc.WaitForExit(30_000);

        // Parse text output
        string? currentLevel = null;
        string? currentTime = null;
        string? currentSource = null;
        string? currentId = null;

        foreach (string rawLine in output.Split('\n'))
        {
            string line = rawLine.Trim();
            if (line.StartsWith("Level:", StringComparison.OrdinalIgnoreCase))
            {
                currentLevel = line.Substring("Level:".Length).Trim();
            }
            else if (line.StartsWith("Date:", StringComparison.OrdinalIgnoreCase))
            {
                currentTime = line.Substring("Date:".Length).Trim();
            }
            else if (line.StartsWith("Source:", StringComparison.OrdinalIgnoreCase))
            {
                currentSource = line.Substring("Source:".Length).Trim();
            }
            else if (line.StartsWith("Event ID:", StringComparison.OrdinalIgnoreCase))
            {
                currentId = line.Substring("Event ID:".Length).Trim();
            }
            else if (line == "" && currentLevel != null)
            {
                // End of an event block
                bool isError = currentLevel.Contains("Error", StringComparison.OrdinalIgnoreCase)
                            || currentLevel.Contains("Critical", StringComparison.OrdinalIgnoreCase);
                bool isWarning = currentLevel.Contains("Warning", StringComparison.OrdinalIgnoreCase);

                if (isError) errorCount++;
                if (isWarning) warningCount++;

                if (isError && recentErrors.Count < 20)
                {
                    recentErrors.Add($"  [{currentTime}] Source={currentSource}, EventID={currentId}");
                }

                currentLevel = null;
                currentTime = null;
                currentSource = null;
                currentId = null;
            }
        }

        sb.AppendLine($"  Errors:   {errorCount}");
        sb.AppendLine($"  Warnings: {warningCount}");

        if (recentErrors.Count > 0)
        {
            sb.AppendLine($"  Top {recentErrors.Count} newest Error events:");
            foreach (var err in recentErrors)
                sb.AppendLine(err);
        }
    }

    // ─────────────────────────────────────────────────────────────
    //  E4: CrossFire logs
    // ─────────────────────────────────────────────────────────────

    private static void CollectCrossFireLogs(string tempDir, DateTime sinceUtc, DateTime untilUtc, BundleManifest manifest)
    {
        string[] crossFireRoots =
        [
            @"C:\Program Files (x86)\Tyco\CrossFire\Logging",
            @"C:\Program Files (x86)\JCI\CrossFire\Logging"
        ];

        string cfDir = Path.Combine(tempDir, "CrossFire");
        bool anyFound = false;

        foreach (string root in crossFireRoots)
        {
            if (!Directory.Exists(root))
                continue;

            anyFound = true;
            // Use a subfolder name based on the vendor
            string vendorName = root.Contains("Tyco", StringComparison.OrdinalIgnoreCase) ? "Tyco" : "JCI";
            string destDir = Path.Combine(cfDir, vendorName);
            Directory.CreateDirectory(destDir);

            try
            {
                long bytesCopied = CopyCrossFireFiles(root, destDir, sinceUtc, untilUtc, out bool wasCapped);

                manifest.Entries.Add(new BundleManifestEntry
                {
                    Source = $"CrossFire logs ({vendorName})",
                    ArchivePath = $"CrossFire/{vendorName}/",
                    BytesCopied = bytesCopied
                });

                if (wasCapped)
                {
                    string notePath = Path.Combine(destDir, "_TRUNCATED.txt");
                    File.WriteAllText(notePath,
                        $"CrossFire log collection was capped at {CrossFireCapBytes / (1024 * 1024)} MB.\n" +
                        "Newest files were included first. Some older files were omitted.");
                    manifest.Errors.Add($"CrossFire ({vendorName}): collection capped at {CrossFireCapBytes / (1024 * 1024)} MB");
                }
            }
            catch (Exception ex)
            {
                manifest.Errors.Add($"CrossFire ({vendorName}) collection failed: {ex.Message}");
            }
        }

        if (!anyFound)
        {
            Directory.CreateDirectory(cfDir);
            string notePath = Path.Combine(cfDir, "_NOT_FOUND.txt");
            File.WriteAllText(notePath,
                "No CrossFire logging directories were found at:\n" +
                string.Join("\n", crossFireRoots));
            manifest.Entries.Add(new BundleManifestEntry
            {
                Source = "CrossFire logs",
                ArchivePath = "CrossFire/_NOT_FOUND.txt",
                BytesCopied = new FileInfo(notePath).Length
            });
        }
    }

    private static long CopyCrossFireFiles(string sourceDir, string destDir, DateTime sinceUtc, DateTime untilUtc, out bool wasCapped)
    {
        wasCapped = false;
        long totalBytes = 0;

        // Get all files, filter by time range, sort newest first
        var files = Directory.GetFiles(sourceDir, "*", SearchOption.AllDirectories)
            .Select(f => new FileInfo(f))
            .Where(fi => fi.LastWriteTimeUtc >= sinceUtc && fi.LastWriteTimeUtc <= untilUtc)
            .OrderByDescending(fi => fi.LastWriteTimeUtc)
            .ToList();

        foreach (var fi in files)
        {
            if (totalBytes + fi.Length > CrossFireCapBytes)
            {
                wasCapped = true;
                break;
            }

            // Preserve relative directory structure
            string relativePath = Path.GetRelativePath(sourceDir, fi.FullName);
            string destFile = Path.Combine(destDir, relativePath);
            string? destSubDir = Path.GetDirectoryName(destFile);
            if (destSubDir != null && !Directory.Exists(destSubDir))
                Directory.CreateDirectory(destSubDir);

            try
            {
                File.Copy(fi.FullName, destFile, overwrite: true);
                totalBytes += fi.Length;
            }
            catch
            {
                // Skip files we can't read (locked, permissions)
            }
        }

        return totalBytes;
    }

    // ─────────────────────────────────────────────────────────────
    //  E5: Manifest
    // ─────────────────────────────────────────────────────────────

    private static void WriteManifest(string tempDir, BundleManifest manifest)
    {
        string dest = Path.Combine(tempDir, "bundle_manifest.json");
        string json = JsonSerializer.Serialize(manifest, DiagJsonContext.Default.BundleManifest);
        File.WriteAllText(dest, json);
    }

    private static string SafeGet(Func<string> getter)
    {
        try { return getter() ?? "N/A"; }
        catch { return "N/A"; }
    }
}

/// <summary>
/// Source-generated JSON context for diagnostics types (trim-safe).
/// </summary>
[JsonSourceGenerationOptions(WriteIndented = true)]
[JsonSerializable(typeof(BundleManifest))]
[JsonSerializable(typeof(SystemInfoData))]
internal partial class DiagJsonContext : JsonSerializerContext { }

using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Diagnostics;

namespace ProcDumpMonitor;

public enum TargetType
{
    Process,
    Service
}

// Source-generated JSON context for trim-safe serialization (Config only)
[JsonSourceGenerationOptions(WriteIndented = true, DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingDefault)]
[JsonSerializable(typeof(Config))]
internal partial class ConfigJsonContext : JsonSerializerContext { }

// Additional source-gen context for non-config types (always write all fields)
[JsonSourceGenerationOptions(WriteIndented = true)]
[JsonSerializable(typeof(HealthStatus))]
[JsonSerializable(typeof(WebhookPayload))]
[JsonSerializable(typeof(CliStatusOutput))]
[JsonSerializable(typeof(OneShotResult))]
internal partial class AppJsonContext : JsonSerializerContext { }

/// <summary>JSON model for --status CLI output.</summary>
public class CliStatusOutput
{
    public string TaskName { get; set; } = "";
    public string MachineName { get; set; } = "";
    public bool Exists { get; set; }
    public string State { get; set; } = "";
    public string LastRunTime { get; set; } = "";
    public string LastRunResult { get; set; } = "";
    public string NextRunTime { get; set; } = "";
}

public class Config
{
    public const int CurrentVersion = 3;

    // ── Schema version (0 = v1/unversioned; 2 = current) ──
    public int ConfigVersion { get; set; }

    // Target
    public string TargetName { get; set; } = "";
    public TargetType TargetType { get; set; } = TargetType.Process;

    // ProcDump
    public string ProcDumpPath { get; set; } = "";
    public string DumpDirectory { get; set; } = "";
    public string DumpType { get; set; } = "Full";       // Full, MiniPlus, Mini
    public bool DumpOnException { get; set; } = true;     // -e
    public bool DumpOnTerminate { get; set; } = true;     // -t
    public bool UseClone { get; set; } = true;            // -r
    public int MaxDumps { get; set; } = 1;
    public int RestartDelaySeconds { get; set; } = 5;

    // ── Scenario preset (V3) ──
    public string Scenario { get; set; } = "";            // "" = Custom/legacy

    // ── Additional operational flags (V3) ──
    public bool AvoidOutage { get; set; }                  // -a
    public bool OverwriteExisting { get; set; }            // -o
    public bool WaitForProcess { get; set; } = true;       // -w (was implicit pre-V3)
    public bool CpuPerUnit { get; set; }                   // -u
    public int CpuDurationSeconds { get; set; }            // 0 = off; -s <N>

    // ── Advanced ProcDump triggers (F7) ──
    public int CpuThreshold { get; set; }                 // 0 = off; 1-100 → -c <N>
    public int CpuLowThreshold { get; set; }              // 0 = off; 1-100 → -cl <N>
    public int MemoryCommitMB { get; set; }               // 0 = off; → -m <N>
    public int HangWindowSeconds { get; set; }            // 0 = off; >0 → -h

    // ── Advanced options (V3, behind toggle) ──
    public string PerformanceCounter { get; set; } = "";   // -p <counter>
    public string PerfCounterThreshold { get; set; } = ""; // -pl <threshold>
    public string ExceptionFilterInclude { get; set; } = "";// -f <filter>
    public string ExceptionFilterExclude { get; set; } = "";// -fx <filter>
    public bool WerIntegration { get; set; }               // -wer
    public int AvoidTerminateTimeout { get; set; }         // 0 = off; -at <N>

    // ── Disk-space guard (F4) ──
    public long MinFreeDiskMB { get; set; } = 5120;       // 5 GB default

    // ── Dump stability check (F1) ──
    public int DumpStabilityTimeoutSeconds { get; set; } = 30;
    public int DumpStabilityPollSeconds { get; set; } = 2;

    // ── Log rotation (F8) ──
    public int MaxLogSizeMB { get; set; } = 10;
    public int MaxLogFiles { get; set; } = 5;

    // ── Dump retention (F9) ──
    public int DumpRetentionDays { get; set; }             // 0 = disabled
    public double DumpRetentionMaxGB { get; set; }         // 0 = disabled

    // Task
    public string TaskName { get; set; } = "ProcDump Monitor";

    // ── Email (F5: multi-recipient) ──
    public bool EmailEnabled { get; set; }
    public string SmtpServer { get; set; } = "";
    public int SmtpPort { get; set; } = 25;
    public bool UseSsl { get; set; } = false;
    public string FromAddress { get; set; } = "";
    public string ToAddress { get; set; } = ""; // semicolon-delimited
    public string CcAddress { get; set; } = "";                       // semicolon-delimited
    public string SmtpUsername { get; set; } = "";
    public string EncryptedPasswordBlob { get; set; } = "";           // Base64-encoded DPAPI blob

    // ── Webhook (F10) ──
    public bool WebhookEnabled { get; set; }
    public string WebhookUrl { get; set; } = "";

    // ----- helpers -----

    private static readonly byte[] Entropy = Encoding.UTF8.GetBytes("ProcDumpMonitor-SMTP-v1");

    /// <summary>Encrypt a plaintext password with DPAPI (LocalMachine scope).</summary>
    public void SetPassword(string plaintext)
    {
        if (string.IsNullOrEmpty(plaintext))
        {
            EncryptedPasswordBlob = "";
            return;
        }
        byte[] data = Encoding.UTF8.GetBytes(plaintext);
        byte[] encrypted = ProtectedData.Protect(data, Entropy, DataProtectionScope.LocalMachine);
        EncryptedPasswordBlob = Convert.ToBase64String(encrypted);
    }

    /// <summary>Decrypt the stored DPAPI blob. Returns empty string on failure.</summary>
    public string GetPassword()
    {
        if (string.IsNullOrEmpty(EncryptedPasswordBlob))
            return "";
        try
        {
            byte[] encrypted = Convert.FromBase64String(EncryptedPasswordBlob);
            byte[] data = ProtectedData.Unprotect(encrypted, Entropy, DataProtectionScope.LocalMachine);
            return Encoding.UTF8.GetString(data);
        }
        catch
        {
            return "";
        }
    }

    /// <summary>Build ProcDump arguments string (for display / copy).</summary>
    public string BuildProcDumpArgs()
    {
        var args = new List<string> { "-accepteula" };

        // Dump type
        switch (DumpType)
        {
            case "Full": args.Add("-ma"); break;
            case "MiniPlus": args.Add("-mp"); break;
            case "Mini": args.Add("-mm"); break;
            case "ThreadDump": args.Add("-mt"); break;
        }

        // Triggers
        if (DumpOnException) args.Add("-e");
        if (DumpOnTerminate) args.Add("-t");
        if (HangWindowSeconds > 0) args.Add("-h");

        // Operational
        if (UseClone) args.Add("-r");
        if (AvoidOutage) args.Add("-a");
        if (OverwriteExisting) args.Add("-o");

        // CPU
        if (CpuThreshold > 0) args.Add($"-c {CpuThreshold}");
        if (CpuLowThreshold > 0) args.Add($"-cl {CpuLowThreshold}");
        if (CpuDurationSeconds > 0) args.Add($"-s {CpuDurationSeconds}");
        if (CpuPerUnit) args.Add("-u");

        // Memory
        if (MemoryCommitMB > 0) args.Add($"-m {MemoryCommitMB}");

        // Advanced
        if (!string.IsNullOrWhiteSpace(PerformanceCounter))
            args.Add($"-p \"{PerformanceCounter}\"");
        if (!string.IsNullOrWhiteSpace(PerfCounterThreshold))
            args.Add($"-pl \"{PerfCounterThreshold}\"");
        if (!string.IsNullOrWhiteSpace(ExceptionFilterInclude))
            args.Add($"-f \"{ExceptionFilterInclude}\"");
        if (!string.IsNullOrWhiteSpace(ExceptionFilterExclude))
            args.Add($"-fx \"{ExceptionFilterExclude}\"");
        if (WerIntegration) args.Add("-wer");
        if (AvoidTerminateTimeout > 0) args.Add($"-at {AvoidTerminateTimeout}");

        // Count
        args.Add($"-n {MaxDumps}");

        // Target
        if (WaitForProcess) args.Add("-w");

        string target = TargetName; 
        if (TargetType == TargetType.Process &&
            !string.IsNullOrWhiteSpace(target) && 
            !target.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
        {
            target += ".exe";
        }

        args.Add(target);
        
        args.Add($"\"{DumpDirectory}\"");

        return string.Join(" ", args);
    }

    // ----- persistence -----

    public static string DefaultConfigPath => AppPaths.ConfigPath;

    public void Save(string? path = null)
    {
        path ??= DefaultConfigPath;

        // Back up before overwriting if migrating
        ConfigMigrator.BackupIfNeeded(path);

        // Always stamp the current schema version
        ConfigVersion = CurrentVersion;

        string json = JsonSerializer.Serialize(this, ConfigJsonContext.Default.Config);
        File.WriteAllText(path, json);
    }

    public static Config Load(string? path = null)
    {
        path ??= DefaultConfigPath;
        if (!File.Exists(path))
            return new Config { ConfigVersion = CurrentVersion, Scenario = "Crash capture" };
        try
        {
            string json = File.ReadAllText(path);
            var cfg = ConfigMigrator.Migrate(json);
            cfg.NormalizeTargetName();
            return cfg;
        }
        catch
        {
            return new Config { ConfigVersion = CurrentVersion, Scenario = "Crash capture" };
        }
    }

    /// <summary>
    /// Normalize legacy TargetName (short/friendly) to full process image name if possible.
    /// </summary>
    public void NormalizeTargetName()
    {
        if (!string.IsNullOrWhiteSpace(TargetName) && !TargetName.Contains(".") && TargetType == TargetType.Process)
        {
            try
            {
                var match = Process.GetProcesses()
                    .Select(p => {
                        try { return System.IO.Path.GetFileNameWithoutExtension(p.MainModule?.ModuleName ?? p.ProcessName); } catch { return p.ProcessName; }
                    })
                    .FirstOrDefault(n => n.Equals(TargetName, StringComparison.OrdinalIgnoreCase) || n.StartsWith(TargetName, StringComparison.OrdinalIgnoreCase));
                if (!string.IsNullOrEmpty(match))
                {
                    Logger.Log($"[Config] Migrated legacy TargetName '{TargetName}' to '{match}'");
                    TargetName = match;
                }
                else
                {
                    Logger.Log($"[Config] Could not resolve legacy TargetName '{TargetName}' to a running process.");
                }
            }
            catch { }
        }
    }
}

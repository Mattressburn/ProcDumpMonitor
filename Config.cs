using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace ProcDumpMonitor;

public class Config
{
    // Target
    public string TargetName { get; set; } = "SoftwareHouse.CrossFire.Server";

    // ProcDump
    public string ProcDumpPath { get; set; } = "";
    public string DumpDirectory { get; set; } = "";
    public string DumpType { get; set; } = "Full";       // Full, MiniPlus, Mini
    public bool DumpOnException { get; set; } = true;     // -e
    public bool DumpOnTerminate { get; set; } = true;     // -t
    public bool UseClone { get; set; } = true;            // -r
    public int MaxDumps { get; set; } = 1;
    public int RestartDelaySeconds { get; set; } = 5;

    // Task
    public string TaskName { get; set; } = "ProcDump Monitor - CrossFire";

    // Email
    public bool EmailEnabled { get; set; } = true;
    public string SmtpServer { get; set; } = "smtp.jci.com";
    public int SmtpPort { get; set; } = 25;
    public bool UseSsl { get; set; } = false;
    public string FromAddress { get; set; } = "matthew.raburn@jci.com";
    public string ToAddress { get; set; } = "matthew.raburn@jci.com";
    public string SmtpUsername { get; set; } = "";
    public string EncryptedPasswordBlob { get; set; } = "";   // Base64-encoded DPAPI blob

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

        switch (DumpType)
        {
            case "Full": args.Add("-ma"); break;
            case "MiniPlus": args.Add("-mp"); break;
            case "Mini": args.Add("-mm"); break;
        }

        if (DumpOnException) args.Add("-e");
        if (DumpOnTerminate) args.Add("-t");
        if (UseClone) args.Add("-r");

        args.Add($"-n {MaxDumps}");
        args.Add($"-w {TargetName}");
        args.Add($"\"{DumpDirectory}\"");

        return string.Join(" ", args);
    }

    // ----- persistence -----

    public static string DefaultConfigPath
    {
        get
        {
            string exeDir = AppContext.BaseDirectory;
            return Path.Combine(exeDir, "config.json");
        }
    }

    private static readonly JsonSerializerOptions JsonOpts = new()
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingDefault
    };

    public void Save(string? path = null)
    {
        path ??= DefaultConfigPath;
        string json = JsonSerializer.Serialize(this, JsonOpts);
        File.WriteAllText(path, json);
    }

    public static Config Load(string? path = null)
    {
        path ??= DefaultConfigPath;
        if (!File.Exists(path))
            return new Config();
        try
        {
            string json = File.ReadAllText(path);
            return JsonSerializer.Deserialize<Config>(json) ?? new Config();
        }
        catch
        {
            return new Config();
        }
    }
}

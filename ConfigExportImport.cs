using System.Text.Json;

namespace ProcDumpMonitor;

/// <summary>
/// Export config with secrets redacted (safe to share / check in).
/// Import config with notifications disabled by default until re-enabled.
/// </summary>
public static class ConfigExportImport
{
    private const string RedactedMarker = "<REDACTED>";

    /// <summary>
    /// Export the config to <paramref name="outputPath"/> with secrets redacted.
    /// EncryptedPasswordBlob and WebhookUrl are replaced with &lt;REDACTED&gt;.
    /// </summary>
    public static void Export(Config cfg, string outputPath)
    {
        // Serialize → deserialize to get a deep copy without sharing references.
        string json = JsonSerializer.Serialize(cfg, ConfigJsonContext.Default.Config);
        var export = JsonSerializer.Deserialize(json, ConfigJsonContext.Default.Config)
            ?? new Config();

        // Redact secrets
        if (!string.IsNullOrEmpty(export.EncryptedPasswordBlob))
            export.EncryptedPasswordBlob = RedactedMarker;

        if (!string.IsNullOrEmpty(export.WebhookUrl))
            export.WebhookUrl = RedactedMarker;

        // Ensure version is stamped
        export.ConfigVersion = Config.CurrentVersion;

        string exportJson = JsonSerializer.Serialize(export, ConfigJsonContext.Default.Config);
        File.WriteAllText(outputPath, exportJson);

        Logger.Log("Config", $"Config exported to {outputPath} (secrets redacted).");
    }

    /// <summary>
    /// Import a config from <paramref name="inputPath"/>.
    /// Notifications (email + webhook) are forced OFF until re-enabled by the user.
    /// Redaction markers are cleared so they don't get saved as real values.
    /// </summary>
    public static Config Import(string inputPath)
    {
        if (!File.Exists(inputPath))
            throw new FileNotFoundException($"Config file not found: {inputPath}");

        string json = File.ReadAllText(inputPath);
        var cfg = ConfigMigrator.Migrate(json);

        // Never auto-enable notifications on import
        cfg.EmailEnabled = false;
        cfg.WebhookEnabled = false;

        // Clear any redaction markers
        if (cfg.EncryptedPasswordBlob == RedactedMarker)
            cfg.EncryptedPasswordBlob = "";
        if (cfg.WebhookUrl == RedactedMarker)
            cfg.WebhookUrl = "";

        Logger.Log("Config", $"Config imported from {inputPath} (notifications disabled until re-enabled).");
        return cfg;
    }
}

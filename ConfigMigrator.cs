using System.Text.Json;
using System.Diagnostics;

namespace ProcDumpMonitor;

/// <summary>
/// Reads raw config JSON, detects version, and applies schema migrations.
/// V1 configs (no configVersion field) are upgraded to V2 with sensible defaults.
/// Newer-than-supported versions trigger a downgrade warning.
/// </summary>
public static class ConfigMigrator
{
    /// <summary>
    /// True after <see cref="Migrate"/> if the config was created by a newer version.
    /// Callers (e.g. MainForm) can show a warning dialog.
    /// </summary>
    public static bool DowngradeWarning { get; private set; }

    /// <summary>
    /// Deserialize JSON to <see cref="Config"/>, applying migrations as needed.
    /// New fields get their property-initializer defaults automatically; this method
    /// detects the version and stamps it to <see cref="Config.CurrentVersion"/>.
    /// </summary>
    public static Config Migrate(string json)
    {
        DowngradeWarning = false;

        Config? cfg;
        try
        {
            cfg = JsonSerializer.Deserialize(json, ConfigJsonContext.Default.Config);
        }
        catch (Exception ex)
        {
            Logger.Log("Config", $"Config deserialization failed: {ex.Message}");
            cfg = null;
        }

        cfg ??= new Config();

        if (cfg.ConfigVersion == 0)
        {
            // V1 config (no configVersion field) → upgrade to V2.
            // New fields already have safe defaults from property initializers.
            Logger.Log("Config", "Migrating config from v1 → v2 (adding defaults for new fields).");
            cfg.ConfigVersion = Config.CurrentVersion;
        }
        else if (cfg.ConfigVersion > Config.CurrentVersion)
        {
            DowngradeWarning = true;
            Logger.Log("Config",
                $"Config version {cfg.ConfigVersion} is newer than supported version {Config.CurrentVersion}. " +
                "Some settings may be ignored.");
        }
        else if (cfg.ConfigVersion < Config.CurrentVersion)
        {
            // V2→V3: new fields use property-initializer defaults.
            // WaitForProcess=true matches V2 behavior (always added -w).
            Logger.Log("Config", $"Migrating config from v{cfg.ConfigVersion} → v{Config.CurrentVersion}.");

            // V2 configs always added -w, so ensure it stays true
            if (cfg.ConfigVersion <= 2)
                cfg.WaitForProcess = true;

            cfg.ConfigVersion = Config.CurrentVersion;
        }

        // Ensure a scenario is always set — "Crash capture" is the safe default.
        // Legacy configs (V1/V2) and imported configs with missing scenario data
        // should never default to "Custom".
        if (string.IsNullOrEmpty(cfg.Scenario))
            cfg.Scenario = "Crash capture";

        // Migrate legacy TargetName (short/friendly) to full process image name if possible
        if (!string.IsNullOrWhiteSpace(cfg.TargetName) && !cfg.TargetName.Contains(".") && cfg.TargetType == TargetType.Process)
        {
            try
            {
                var match = Process.GetProcesses()
                    .Select(p => {
                        try { return System.IO.Path.GetFileNameWithoutExtension(p.MainModule?.ModuleName ?? p.ProcessName); } catch { return p.ProcessName; }
                    })
                    .FirstOrDefault(n => n.Equals(cfg.TargetName, StringComparison.OrdinalIgnoreCase) || n.StartsWith(cfg.TargetName, StringComparison.OrdinalIgnoreCase));
                if (!string.IsNullOrEmpty(match))
                {
                    Logger.Log($"[ConfigMigrator] Migrated legacy TargetName '{cfg.TargetName}' to '{match}'");
                    cfg.TargetName = match;
                }
                else
                {
                    Logger.Log($"[ConfigMigrator] Could not resolve legacy TargetName '{cfg.TargetName}' to a running process.");
                }
            }
            catch { }
        }
        return cfg;
    }

    /// <summary>
    /// Create a backup of the config file before overwriting with a new version.
    /// Called by <see cref="Config.Save"/> when a migration has occurred.
    /// </summary>
    public static void BackupIfNeeded(string configPath)
    {
        try
        {
            if (!File.Exists(configPath))
                return;

            string backupPath = configPath + ".bak";
            File.Copy(configPath, backupPath, overwrite: true);
            Logger.Log("Config", $"Backup saved to {backupPath}");
        }
        catch (Exception ex)
        {
            Logger.Log("Config", $"Backup failed (non-fatal): {ex.Message}");
        }
    }
}

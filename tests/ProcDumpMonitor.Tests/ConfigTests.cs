namespace ProcDumpMonitor.Tests;

public class ConfigTests
{
    // ── e1: Config migration V1 → V2 ──

    [Fact]
    public void ConfigMigrator_UpgradesV1ToV2()
    {
        // V1 config has no ConfigVersion field
        string v1Json = """
        {
            "TargetName": "OldTarget",
            "ProcDumpPath": "procdump.exe",
            "DumpDirectory": "C:\\Dumps"
        }
        """;

        var cfg = ConfigMigrator.Migrate(v1Json);

        Assert.Equal(Config.CurrentVersion, cfg.ConfigVersion);
        Assert.Equal("OldTarget", cfg.TargetName);
        // New fields should have defaults
        Assert.Equal(5120, cfg.MinFreeDiskMB);
        Assert.Equal(10, cfg.MaxLogSizeMB);
        Assert.Equal(30, cfg.DumpStabilityTimeoutSeconds);
    }

    [Fact]
    public void ConfigMigrator_SetsDowngradeWarning_ForNewerVersion()
    {
        string futureJson = """
        {
            "ConfigVersion": 999,
            "TargetName": "Future"
        }
        """;

        var cfg = ConfigMigrator.Migrate(futureJson);

        Assert.True(ConfigMigrator.DowngradeWarning);
        Assert.Equal("Future", cfg.TargetName);
    }

    [Fact]
    public void ConfigMigrator_UpgradesShortTargetName()
    {
        string legacyJson = "{\"TargetName\":\"CrossFire\",\"TargetType\":0}";
        var config = ConfigMigrator.Migrate(legacyJson);
        Assert.False(string.IsNullOrWhiteSpace(config.TargetName));
        Assert.True(config.TargetName.StartsWith("SoftwareHouse.CrossFire", StringComparison.OrdinalIgnoreCase));
    }

    // ── e2: Export/Import round-trip ──

    [Fact]
    public void ConfigExportImport_RoundTrip_RedactsSecrets()
    {
        var cfg = new Config
        {
            TargetName = "Test",
            EmailEnabled = true,
            WebhookEnabled = true,
            WebhookUrl = "https://hooks.example.com/test",
            SmtpUsername = "user",
            // Set blob directly to avoid DPAPI LocalMachine scope requirement
            EncryptedPasswordBlob = "dGVzdC1ibG9i",
        };

        string tempFile = Path.GetTempFileName();
        try
        {
            ConfigExportImport.Export(cfg, tempFile);

            string exported = File.ReadAllText(tempFile);
            // System.Text.Json encodes < > as \u003C \u003E, so check the escaped form
            Assert.Contains(@"\u003CREDACTED\u003E", exported);
            Assert.DoesNotContain("dGVzdC1ibG9i", exported); // blob must not appear
            Assert.DoesNotContain("hooks.example.com", exported); // webhook URL must not appear

            var imported = ConfigExportImport.Import(tempFile);

            Assert.Equal("Test", imported.TargetName);
            Assert.False(imported.EmailEnabled);      // forced OFF on import
            Assert.False(imported.WebhookEnabled);     // forced OFF on import
            Assert.Equal("", imported.EncryptedPasswordBlob); // marker cleared
            Assert.Equal("", imported.WebhookUrl);            // marker cleared
        }
        finally
        {
            File.Delete(tempFile);
        }
    }
}

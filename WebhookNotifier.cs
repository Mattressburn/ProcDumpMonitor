using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace ProcDumpMonitor;

/// <summary>
/// JSON payload compatible with both Microsoft Teams (MessageCard) and Slack
/// incoming webhook formats. Teams uses @type/themeColor/sections; Slack uses text.
/// </summary>
public class WebhookPayload
{
    [JsonPropertyName("@type")]
    public string Type { get; set; } = "MessageCard";

    [JsonPropertyName("summary")]
    public string Summary { get; set; } = "";

    [JsonPropertyName("themeColor")]
    public string ThemeColor { get; set; } = "FF0000";

    [JsonPropertyName("title")]
    public string Title { get; set; } = "";

    [JsonPropertyName("text")]
    public string Text { get; set; } = "";
}

/// <summary>
/// Posts JSON payloads to a configured webhook URL (Teams / Slack style).
/// Has a short timeout and never throws uncaught exceptions — failures are
/// logged and silently skipped so they never block dump capture.
/// </summary>
public class WebhookNotifier : INotifier
{
    private static readonly HttpClient Http = new()
    {
        Timeout = TimeSpan.FromSeconds(15)
    };

    public bool IsEnabled(Config cfg) =>
        cfg.WebhookEnabled && !string.IsNullOrWhiteSpace(cfg.WebhookUrl);

    public void NotifyDump(Config cfg, string dumpFilePath)
    {
        var payload = new WebhookPayload
        {
            Summary = $"Dump created for {cfg.TargetName}",
            ThemeColor = "FF0000",
            Title = $"[ProcDump] Dump created for {cfg.TargetName} on {Environment.MachineName}",
            Text = $"**Target:** {cfg.TargetName}\n\n" +
                   $"**Computer:** {Environment.MachineName}\n\n" +
                   $"**Dump File:** {dumpFilePath}\n\n" +
                   $"**Timestamp:** {DateTime.Now:yyyy-MM-dd HH:mm:ss}"
        };

        Post(cfg.WebhookUrl, payload);
    }

    public void NotifyWarning(Config cfg, string subject, string message)
    {
        var payload = new WebhookPayload
        {
            Summary = subject,
            ThemeColor = "FFAA00",
            Title = subject,
            Text = message
        };

        Post(cfg.WebhookUrl, payload);
    }

    private static void Post(string url, WebhookPayload payload)
    {
        try
        {
            string json = JsonSerializer.Serialize(payload, AppJsonContext.Default.WebhookPayload);
            using var content = new StringContent(json, Encoding.UTF8, "application/json");

            // Sync-over-async is acceptable in the single-threaded monitor loop.
            using var response = Http.PostAsync(url, content).GetAwaiter().GetResult();

            if (!response.IsSuccessStatusCode)
                Logger.Log("Webhook", $"Webhook returned {(int)response.StatusCode}: {response.ReasonPhrase}");
            else
                Logger.Log("Webhook", "Webhook notification sent.");
        }
        catch (TaskCanceledException)
        {
            Logger.Log("Webhook", "Webhook request timed out (15s).");
        }
        catch (HttpRequestException ex)
        {
            Logger.Log("Webhook", $"Webhook request failed: {ex.Message}");
        }
        catch (Exception ex)
        {
            Logger.Log("Webhook", $"Webhook error: {ex.Message}");
        }
    }
}

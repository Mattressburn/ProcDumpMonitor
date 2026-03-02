namespace ProcDumpMonitor;

/// <summary>
/// Abstraction for notification channels (email, webhook, etc.).
/// Implementations must never throw uncaught exceptions — failures are logged and skipped.
/// </summary>
public interface INotifier
{
    /// <summary>Whether this notifier is enabled in the current config.</summary>
    bool IsEnabled(Config cfg);

    /// <summary>Send a dump-captured notification.</summary>
    void NotifyDump(Config cfg, string dumpFilePath);

    /// <summary>Send a warning (e.g., low disk space).</summary>
    void NotifyWarning(Config cfg, string subject, string message);
}

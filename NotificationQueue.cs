using System.Collections.Concurrent;

namespace ProcDumpMonitor;

/// <summary>
/// Non-blocking notification dispatcher. Queues notification work items and
/// processes them on a background thread so the monitor loop is never stalled
/// by slow SMTP or webhook calls.
///
/// Bounded to <see cref="MaxQueueSize"/> items to prevent unbounded memory growth.
/// All exceptions are caught and logged; failures never propagate back to the caller.
/// </summary>
public sealed class NotificationQueue : IDisposable
{
    private readonly BlockingCollection<Action> _queue;
    private readonly Thread _worker;
    private volatile bool _disposed;

    /// <summary>Maximum number of queued notification items before new items are dropped.</summary>
    public int MaxQueueSize { get; }

    public NotificationQueue(int maxQueueSize = 64)
    {
        MaxQueueSize = maxQueueSize;
        _queue = new BlockingCollection<Action>(new ConcurrentQueue<Action>(), maxQueueSize);

        _worker = new Thread(ProcessLoop)
        {
            Name = "NotificationQueue",
            IsBackground = true
        };
        _worker.Start();
    }

    /// <summary>
    /// Enqueue a notification action for background execution.
    /// Returns immediately. If the queue is full the item is dropped and a warning is logged.
    /// </summary>
    public void Enqueue(Action work)
    {
        if (_disposed) return;

        if (!_queue.TryAdd(work))
        {
            Logger.Log("NotifyQ", "Notification queue full; dropping item.");
        }
    }

    /// <summary>
    /// Fire all enabled notifiers for a dump event. Non-blocking.
    /// </summary>
    public void EnqueueDump(Config cfg, INotifier[] notifiers, string dumpFilePath)
    {
        foreach (var notifier in notifiers)
        {
            if (!notifier.IsEnabled(cfg)) continue;
            var n = notifier; // capture for closure
            Enqueue(() =>
            {
                try
                {
                    n.NotifyDump(cfg, dumpFilePath);
                    Logger.Log("NotifyQ", $"{n.GetType().Name}: dump notification sent.");
                }
                catch (Exception ex)
                {
                    Logger.Log("NotifyQ", $"{n.GetType().Name}: dump notification failed: {ex.Message}");
                }
            });
        }
    }

    /// <summary>
    /// Fire all enabled notifiers for a warning event. Non-blocking.
    /// </summary>
    public void EnqueueWarning(Config cfg, INotifier[] notifiers, string subject, string message)
    {
        foreach (var notifier in notifiers)
        {
            if (!notifier.IsEnabled(cfg)) continue;
            var n = notifier;
            Enqueue(() =>
            {
                try
                {
                    n.NotifyWarning(cfg, subject, message);
                }
                catch (Exception ex)
                {
                    Logger.Log("NotifyQ", $"{n.GetType().Name}: warning notification failed: {ex.Message}");
                }
            });
        }
    }

    private void ProcessLoop()
    {
        try
        {
            foreach (var work in _queue.GetConsumingEnumerable())
            {
                try
                {
                    work();
                }
                catch (Exception ex)
                {
                    Logger.Log("NotifyQ", $"Unhandled notification error: {ex.Message}");
                }
            }
        }
        catch (OperationCanceledException)
        {
            // Expected on dispose
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        _queue.CompleteAdding();

        // Give the worker a short time to drain
        _worker.Join(TimeSpan.FromSeconds(5));
        _queue.Dispose();
    }
}

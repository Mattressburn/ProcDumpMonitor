using Xunit;

namespace ProcDumpMonitor.Tests;

public class TaskSchedulerServiceTests
{
    [Fact]
    public void BuildActionPreview_UsesOneshot_WhenFlagSet()
    {
        var cfg = new Config { RemoveTaskAfterSuccessfulDump = true };
        var preview = TaskSchedulerService.BuildActionPreview(cfg);
        Assert.Contains("--oneshot", preview.Arguments);
    }

    [Fact]
    public void BuildActionPreview_UsesMonitor_WhenFlagNotSet()
    {
        var cfg = new Config { RemoveTaskAfterSuccessfulDump = false };
        var preview = TaskSchedulerService.BuildActionPreview(cfg);
        Assert.Contains("--monitor", preview.Arguments);
    }
}

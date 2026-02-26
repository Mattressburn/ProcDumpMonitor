namespace ProcDumpMonitor;

internal static class Program
{
    [STAThread]
    static void Main(string[] args)
    {
        // ── Monitor mode (headless, launched by Scheduled Task) ──
        if (args.Contains("--monitor", StringComparer.OrdinalIgnoreCase))
        {
            string? configPath = null;
            for (int i = 0; i < args.Length - 1; i++)
            {
                if (args[i].Equals("--config", StringComparison.OrdinalIgnoreCase))
                {
                    configPath = args[i + 1];
                    break;
                }
            }

            var cfg = Config.Load(configPath);
            ProcDumpMonitorLoop.Run(cfg);
            return;
        }

        // ── GUI mode ──
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        ApplicationConfiguration.Initialize();
        Application.Run(new MainForm());
    }
}

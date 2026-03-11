using System.Diagnostics;

namespace ProcDumpMonitor;

/// <summary>Wizard Step 5 — Review settings and execute.</summary>
public sealed class ReviewPage : WizardPage
{
    public override string StepTitle => "Review";

    // ── Summary ──
    private readonly RichTextBox _txtSummary = new()
    {
        ReadOnly = true,
        Dock = DockStyle.Fill,
        BorderStyle = BorderStyle.None,
        ScrollBars = RichTextBoxScrollBars.Vertical
    };

    // ── Action buttons ──
    private readonly Button _btnCreateTask = new() { Text = "Create Task", MinimumSize = new Size(140, 38) };
    private readonly Button _btnRunNow = new() { Text = "Run Task Now", MinimumSize = new Size(120, 38), Enabled = false };
    private readonly Button _btnStop = new() { Text = "Stop Task", MinimumSize = new Size(90, 38), Enabled = false };
    private readonly Button _btnRemove = new() { Text = "Remove Task", MinimumSize = new Size(100, 38), Enabled = false };
    private readonly Button _btnOneShot = new() { Text = "Run One-Shot", MinimumSize = new Size(120, 38) };
    private readonly Button _btnSaveOnly = new() { Text = "Save Config Only", MinimumSize = new Size(120, 34) };
    private readonly Button _btnOpenDumpFolder = new() { Text = "Open Dump Folder", MinimumSize = new Size(120, 34) };
    private readonly Button _btnViewLogs = new() { Text = "View Logs", MinimumSize = new Size(90, 34) };
    private readonly Button _btnCopyCmd = new() { Text = "Copy ProcDump Cmd", MinimumSize = new Size(130, 34) };
    private readonly Button _btnOpenTaskScheduler = new() { Text = "Open Task Scheduler", MinimumSize = new Size(130, 34) };

    // ── Utilities (convenience shortcuts, not primary owners) ──
    private readonly Button _btnSupportDiag = new() { Text = "Support Diagnostics...", MinimumSize = new Size(120, 30) };
    private readonly ToolTip _toolTip = new();

    // ── Status banner ──
    private readonly Panel _pnlStatus = new() { Name = "StatusBanner", Dock = DockStyle.Bottom, Height = 36, Padding = new Padding(12, 6, 12, 6) };
    private readonly Label _lblStatus = new() { AutoSize = true, Dock = DockStyle.Left, ForeColor = Color.White };

    // ── Status log ──
    private readonly RichTextBox _txtLog = new()
    {
        ReadOnly = true,
        Dock = DockStyle.Bottom,
        Height = 90,
        BorderStyle = BorderStyle.None,
        ScrollBars = RichTextBoxScrollBars.Vertical
    };

    private Config _cfg = new();
    private bool _taskExisted;
    private readonly System.Windows.Forms.Timer _pollTimer = new() { Interval = 3000 };
    private bool _polling;

    public ReviewPage()
    {
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 5,
            Margin = Padding.Empty
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        // Row 0 -- Summary (fills space)
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.Controls.Add(_txtSummary, 0, 0);

        // Row 1 -- Action buttons (task and dump controls only)
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var btnFlow = MakeButtonFlow(
            _btnCreateTask, _btnRunNow, _btnStop, _btnRemove, _btnOneShot,
            _btnSaveOnly, _btnOpenDumpFolder, _btnViewLogs, _btnCopyCmd, _btnOpenTaskScheduler);
        layout.Controls.Add(btnFlow, 0, 1);

        // Row 2 -- Utilities (convenience shortcuts to features that live elsewhere)
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var utilFlow = new FlowLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            WrapContents = true,
            FlowDirection = FlowDirection.LeftToRight,
            Padding = Padding.Empty,
            Margin = new Padding(0, 2, 0, 0)
        };
        var utilLabel = new Label
        {
            Text = "Utilities:",
            AutoSize = true,
            Anchor = AnchorStyles.Left,
            Margin = new Padding(0, 7, 4, 0)
        };
        utilLabel.Font = new Font(utilLabel.Font.FontFamily, utilLabel.Font.Size - 0.5f, FontStyle.Italic);
        _btnSupportDiag.Font = new Font(_btnSupportDiag.Font.FontFamily, _btnSupportDiag.Font.Size - 0.5f);
        _toolTip.SetToolTip(_btnSupportDiag,
            "Shortcut to Support Diagnostics. This feature is available at any time from Tools or the tray menu.");
        utilFlow.Controls.Add(utilLabel);
        utilFlow.Controls.Add(_btnSupportDiag);
        layout.Controls.Add(utilFlow, 0, 2);

        // Row 3 -- Status banner
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _pnlStatus.Controls.Add(_lblStatus);
        layout.Controls.Add(_pnlStatus, 0, 3);

        // Row 4 -- Log
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 90));
        layout.Controls.Add(_txtLog, 0, 4);

        Controls.Add(layout);

        SetStatusBanner("idle", "Ready.");
        WireEvents();
        _pollTimer.Tick += PollTaskStatus;
    }

    private void WireEvents()
    {
        _btnCreateTask.Click += async (_, _) =>
        {
            SetStatusBanner("working", _taskExisted ? "Updating scheduled task…" : "Creating scheduled task…");
            AppendLog(_taskExisted ? "Updating task…" : "Creating task…");
            _btnCreateTask.Enabled = false;
            try
            {
                bool existed = false;
                await Task.Run(() =>
                {
                    _cfg.Save();
                    existed = TaskSchedulerService.InstallOrUpdate(_cfg);
                });
                string verb = existed ? "updated" : "created";
                SetStatusBanner("success", $"✓ Scheduled task '{_cfg.TaskName}' {verb} successfully.");
                AppendLog($"Task '{_cfg.TaskName}' {verb}.");
                _taskExisted = true;
                UpdateTaskButtons(true);
            }
            catch (Exception ex)
            {
                SetStatusBanner("error", $"✖ {ex.Message}");
                AppendLog($"FAILED: {ex.Message}");
                Logger.Log("ReviewPage", $"Create/Update failed: {ex}");
            }
            finally
            {
                _btnCreateTask.Enabled = true;
            }
        };

        _btnRunNow.Click += async (_, _) =>
        {
            try
            {
                SetStatusBanner("working", "Starting task…");
                await Task.Run(() => TaskSchedulerService.StartNow(_cfg.TaskName));
                SetStatusBanner("success", $"✓ Task '{_cfg.TaskName}' started.");
                AppendLog("Task started.");
            }
            catch (Exception ex)
            {
                SetStatusBanner("error", $"✖ {ex.Message}");
                AppendLog($"Start failed: {ex.Message}");
            }
        };

        _btnStop.Click += async (_, _) =>
        {
            try
            {
                SetStatusBanner("working", "Stopping task…");
                await Task.Run(() => TaskSchedulerService.StopTask(_cfg.TaskName));
                SetStatusBanner("success", $"✓ Task '{_cfg.TaskName}' stopped.");
                AppendLog("Task stopped.");
            }
            catch (Exception ex)
            {
                SetStatusBanner("error", $"✖ {ex.Message}");
                AppendLog($"Stop failed: {ex.Message}");
            }
        };

        _btnRemove.Click += async (_, _) =>
        {
            try
            {
                SetStatusBanner("working", "Removing task…");
                await Task.Run(() => TaskSchedulerService.RemoveTask(_cfg.TaskName));
                SetStatusBanner("success", $"✓ Task '{_cfg.TaskName}' removed.");
                AppendLog("Task removed.");
                _taskExisted = false;
                UpdateTaskButtons(false);
                _btnCreateTask.Text = "Create Task";
            }
            catch (Exception ex)
            {
                SetStatusBanner("error", $"✖ {ex.Message}");
                AppendLog($"Remove failed: {ex.Message}");
            }
        };

        _btnSaveOnly.Click += (_, _) =>
        {
            try
            {
                _cfg.Save();
                SetStatusBanner("success", $"✓ Config saved to {Config.DefaultConfigPath}");
                AppendLog("Config saved.");
            }
            catch (Exception ex)
            {
                SetStatusBanner("error", $"✖ Save failed: {ex.Message}");
            }
        };

        _btnOneShot.Click += async (_, _) =>
        {
            SetStatusBanner("working", "Running one-shot sequence…");
            AppendLog("One-shot: starting…");
            _btnOneShot.Enabled = false;
            _btnCreateTask.Enabled = false;
            try
            {
                var options = new OneShotOptions
                {
                    SimulateDump = true,
                    SimulateTask = false,
                    NoEmail = false
                };

                ITaskSchedulerOps taskOps = new RealTaskSchedulerOps();
                IProcDumpRunner procDump = new SimulatedProcDumpRunner();
                IEmailSender email = new RealEmailSender();

                using var cts = new CancellationTokenSource();
                var runner = new OneShotRunner(_cfg, taskOps, procDump, email, options);

                var result = await Task.Run(() => runner.Execute(cts.Token));

                foreach (var step in result.Steps)
                    AppendLog(step);

                if (result.Success)
                {
                    SetStatusBanner("success", "✓ One-shot completed. Task created, email sent, task removed.");
                    _taskExisted = false;
                    UpdateTaskButtons(false);
                    _btnCreateTask.Text = "Create Task";
                }
                else
                {
                    SetStatusBanner("error", $"✖ One-shot failed: {result.Error}");
                }
            }
            catch (Exception ex)
            {
                SetStatusBanner("error", $"✖ {ex.Message}");
                AppendLog($"One-shot error: {ex.Message}");
            }
            finally
            {
                _btnOneShot.Enabled = true;
                _btnCreateTask.Enabled = true;
            }
        };

        _btnOpenDumpFolder.Click += (_, _) =>
        {
            if (Directory.Exists(_cfg.DumpDirectory))
                Process.Start("explorer.exe", _cfg.DumpDirectory);
            else
                SetStatusBanner("error", "Dump directory does not exist yet.");
        };

        _btnViewLogs.Click += (_, _) =>
        {
            if (File.Exists(Logger.LogPath))
                Process.Start("notepad.exe", Logger.LogPath);
            else
                SetStatusBanner("error", "Log file does not exist yet.");
        };

        _btnCopyCmd.Click += (_, _) =>
        {
            string cmd = $"\"{_cfg.ProcDumpPath}\" {_cfg.BuildProcDumpArgs()}";
            try
            {
                Clipboard.SetText(cmd);
                SetStatusBanner("success", "✓ ProcDump command copied to clipboard.");
                AppendLog($"Copied: {cmd}");
            }
            catch (Exception ex)
            {
                SetStatusBanner("error", $"✖ Clipboard error: {ex.Message}");
            }
        };

        _btnOpenTaskScheduler.Click += (_, _) =>
        {
            try { Process.Start(new ProcessStartInfo("taskschd.msc") { UseShellExecute = true }); }
            catch (Exception ex) { SetStatusBanner("error", $"Cannot open Task Scheduler: {ex.Message}"); }
        };

        _btnSupportDiag.Click += (_, _) =>
        {
            // Delegate to MainForm -- diagnostics are not owned by the wizard.
            if (FindForm() is MainForm main)
                main.RunSupportDiagnosticsFromGui();
        };
    }

    public override void OnEnter(Config cfg)
    {
        _cfg = cfg;

        // Detect existing task
        try
        {
            var info = TaskSchedulerService.GetDetailedStatus(cfg.TaskName);
            _taskExisted = info.Exists;
            _btnCreateTask.Text = info.Exists ? "Update Task" : "Create Task";
            UpdateTaskButtons(info.Exists);
        }
        catch
        {
            _taskExisted = false;
            _btnCreateTask.Text = "Create Task";
            UpdateTaskButtons(false);
        }

        BuildSummary();
        SetStatusBanner("idle", _taskExisted
            ? $"Task '{cfg.TaskName}' exists — click Update Task to apply changes."
            : "Ready to create scheduled task.");

        _pollTimer.Start();
    }

    public override bool OnLeave(Config cfg)
    {
        _pollTimer.Stop();
        return true;
    }

    public override bool IsValid() => true;

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _pollTimer.Stop();
            _pollTimer.Dispose();
            _toolTip.Dispose();
        }
        base.Dispose(disposing);
    }

    private async void PollTaskStatus(object? sender, EventArgs e)
    {
        if (_polling) return;
        _polling = true;
        try
        {
            var info = await Task.Run(() => TaskSchedulerService.GetDetailedStatus(_cfg.TaskName));
            if (IsDisposed) return;
            Invoke(() =>
            {
                _btnRunNow.Enabled = info.Exists;
                _btnStop.Enabled = info.Exists;
                _btnRemove.Enabled = info.Exists;
                _lblStatus.Text = $"Task state: {info.State}";
            });
        }
        catch { }
        finally
        {
            _polling = false;
        }
    }

    private void UpdateTaskButtons(bool taskExists)
    {
        _btnRunNow.Enabled = taskExists;
        _btnStop.Enabled = taskExists;
        _btnRemove.Enabled = taskExists;
    }

    private void BuildSummary()
    {
        var sb = new System.Text.StringBuilder();

        sb.AppendLine("═══ TARGET ═══");
        sb.AppendLine($"  Process:        {_cfg.TargetName}");
        sb.AppendLine();

        sb.AppendLine("═══ PROCDUMP ═══");
        sb.AppendLine($"  Scenario:       {(string.IsNullOrEmpty(_cfg.Scenario) ? "Custom" : _cfg.Scenario)}");
        sb.AppendLine($"  Path:           {_cfg.ProcDumpPath}");
        sb.AppendLine($"  Dump Directory: {_cfg.DumpDirectory}");
        sb.AppendLine($"  Dump Type:      {_cfg.DumpType}");
        sb.AppendLine($"  Triggers:       {FormatTriggers()}");
        sb.AppendLine($"  Max Dumps:      {_cfg.MaxDumps}");
        sb.AppendLine($"  Restart Delay:  {_cfg.RestartDelaySeconds}s");
        sb.AppendLine($"  Min Free Disk:  {_cfg.MinFreeDiskMB} MB");

        // Bitness detection
        try
        {
            string procDumpDir = Path.GetDirectoryName(_cfg.ProcDumpPath) ?? AppPaths.InstallDir;
            var bitness = ProcDumpBitnessResolver.Resolve(_cfg.TargetName, procDumpDir);
            sb.AppendLine($"  Bitness:        {bitness.Summary}");
            if (bitness.Warning != null)
                sb.AppendLine($"  ⚠ {bitness.Warning}");
        }
        catch { sb.AppendLine("  Bitness:        (detection unavailable)"); }
        sb.AppendLine();

        sb.AppendLine("═══ SCHEDULED TASK ═══");
        sb.AppendLine($"  Task Name:      {_cfg.TaskName}");
        sb.AppendLine($"  Action:         {(_taskExisted ? "UPDATE existing" : "CREATE new")}");
        try
        {
            var preview = TaskSchedulerService.BuildActionPreview(_cfg);
            sb.AppendLine($"  Command:        \"{preview.ExePath}\" {preview.Arguments}");
        }
        catch { /* ignore preview errors */ }
        sb.AppendLine();

        sb.AppendLine("═══ NOTIFICATIONS ═══");
        if (_cfg.EmailEnabled)
        {
            sb.AppendLine($"  Email:          ON → {_cfg.ToAddress}");
            sb.AppendLine($"  SMTP:           {_cfg.SmtpServer}:{_cfg.SmtpPort} (SSL={_cfg.UseSsl})");
            if (!string.IsNullOrWhiteSpace(_cfg.CcAddress))
                sb.AppendLine($"  CC:             {_cfg.CcAddress}");
        }
        else
        {
            sb.AppendLine("  Email:          OFF");
        }
        sb.AppendLine($"  Webhook:        {(_cfg.WebhookEnabled ? $"ON → {_cfg.WebhookUrl}" : "OFF")}");
        sb.AppendLine();

        sb.AppendLine("═══ MAINTENANCE ═══");
        sb.AppendLine($"  Log Rotation:   {_cfg.MaxLogSizeMB} MB × {_cfg.MaxLogFiles} files");
        sb.AppendLine($"  Dump Retention: {(_cfg.DumpRetentionDays > 0 ? $"{_cfg.DumpRetentionDays} days" : "disabled")}");
        sb.AppendLine($"  Max Dump Size:  {(_cfg.DumpRetentionMaxGB > 0 ? $"{_cfg.DumpRetentionMaxGB} GB" : "disabled")}");
        sb.AppendLine($"  Stability:      {_cfg.DumpStabilityTimeoutSeconds}s");

        _txtSummary.Text = sb.ToString();
    }

    private string FormatTriggers()
    {
        var parts = new List<string>();
        if (_cfg.DumpOnException) parts.Add("Exception (-e)");
        if (_cfg.DumpOnTerminate) parts.Add("Terminate (-t)");
        if (_cfg.HangWindowSeconds > 0) parts.Add("Hang (-h)");
        if (_cfg.UseClone) parts.Add("Clone (-r)");
        if (_cfg.AvoidOutage) parts.Add("Avoid outage (-a)");
        if (_cfg.CpuThreshold > 0) parts.Add($"CPU ≥{_cfg.CpuThreshold}%");
        if (_cfg.CpuLowThreshold > 0) parts.Add($"CPU ≤{_cfg.CpuLowThreshold}%");
        if (_cfg.CpuDurationSeconds > 0) parts.Add($"Duration {_cfg.CpuDurationSeconds}s");
        if (_cfg.CpuPerUnit) parts.Add("Per-CPU (-u)");
        if (_cfg.MemoryCommitMB > 0) parts.Add($"Mem ≥{_cfg.MemoryCommitMB} MB");
        if (_cfg.WerIntegration) parts.Add("WER (-wer)");
        return parts.Count > 0 ? string.Join(", ", parts) : "(none)";
    }

    private void SetStatusBanner(string state, string message)
    {
        if (InvokeRequired) { Invoke(() => SetStatusBanner(state, message)); return; }

        _pnlStatus.BackColor = state switch
        {
            "success" => ColorTranslator.FromHtml("#1F6F3A"),
            "error" => ColorTranslator.FromHtml("#8B1E1E"),
            "working" => ThemeManager.Accent,
            _ => ColorTranslator.FromHtml("#2D2D30")
        };
        _lblStatus.Text = message;
        _lblStatus.ForeColor = Color.White;
        _lblStatus.BackColor = _pnlStatus.BackColor;
    }

    private void AppendLog(string message)
    {
        if (InvokeRequired) { Invoke(() => AppendLog(message)); return; }
        _txtLog.Text = $"[{DateTime.Now:HH:mm:ss}] {message}\n{_txtLog.Text}";
    }
}

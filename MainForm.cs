using System.Diagnostics;
using System.Security.Principal;

namespace ProcDumpMonitor;

public class MainForm : Form
{
    // ── Target ──
    private readonly TextBox _txtTarget = new();

    // ── ProcDump ──
    private readonly TextBox _txtProcDumpPath = new();
    private readonly Button _btnBrowseProcDump = new() { Text = "Browse…" };
    private readonly TextBox _txtDumpDir = new();
    private readonly Button _btnBrowseDumpDir = new() { Text = "Browse…" };
    private readonly ComboBox _cboDumpType = new() { DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly ThemedCheckBox _chkException = new() { Text = "Dump on unhandled exception (-e)" };
    private readonly ThemedCheckBox _chkTerminate = new() { Text = "Dump on terminate (-t)" };
    private readonly ThemedCheckBox _chkClone = new() { Text = "Use clone (-r)" };
    private readonly NumericUpDown _nudMaxDumps = new() { Minimum = 1, Maximum = 100, Value = 1 };
    private readonly NumericUpDown _nudRestartDelay = new() { Minimum = 1, Maximum = 600, Value = 5 };

    // ── Task ──
    private readonly TextBox _txtTaskName = new();
    private readonly Button _btnInstall = new() { Text = "Create / Update Task" };
    private readonly Button _btnRunNow = new() { Text = "Run Task Now" };
    private readonly Button _btnStop = new() { Text = "Stop Task" };
    private readonly Button _btnRemove = new() { Text = "Remove Task" };
    private readonly Button _btnOpenDumpFolder = new() { Text = "Open Dump Folder" };
    private readonly Button _btnViewLogs = new() { Text = "View Logs" };

    // Task – banner
    private readonly Panel _pnlBanner = new() { Name = "BannerPanel", Visible = false, AutoSize = true, Dock = DockStyle.Top, Padding = new Padding(8, 6, 8, 6) };
    private readonly Label _lblBannerText = new() { AutoSize = true, ForeColor = Color.White, Dock = DockStyle.Left };
    private readonly Button _btnBannerAction1 = new() { AutoSize = true, FlatStyle = FlatStyle.Flat };
    private readonly Button _btnBannerAction2 = new() { AutoSize = true, FlatStyle = FlatStyle.Flat };
    private readonly Button _btnBannerDismiss = new() { Text = "✕", AutoSize = true, FlatStyle = FlatStyle.Flat, ForeColor = Color.White };
    private System.Windows.Forms.Timer _bannerAutoHideTimer = new();

    // Task – status info
    private readonly Label _lblTaskExists = new() { AutoSize = true };
    private readonly Label _lblTaskState = new() { AutoSize = true };
    private readonly Label _lblTaskLastRun = new() { AutoSize = true };
    private readonly Label _lblTaskLastResult = new() { AutoSize = true };
    private readonly Label _lblTaskNextRun = new() { AutoSize = true };
    private readonly Button _btnRefreshStatus = new() { Text = "Refresh" };

    // Task – action preview
    private readonly TextBox _txtActionPreview = new() { ReadOnly = true, Multiline = true, ScrollBars = ScrollBars.Vertical, Height = 52 };
    private readonly Button _btnCopyAction = new() { Text = "Copy Task Action" };

    // Task – navigation
    private readonly Button _btnOpenTaskScheduler = new() { Text = "Open Task Scheduler" };
    private readonly Button _btnViewHistory = new() { Text = "View Task History" };
    private readonly Button _btnEnableHistory = new() { Text = "Enable Task History" };

    // ── Email ──
    private readonly ThemedCheckBox _chkEmailEnabled = new() { Text = "Enable email notifications" };
    private readonly TextBox _txtSmtpServer = new();
    private readonly NumericUpDown _nudSmtpPort = new() { Minimum = 1, Maximum = 65535, Value = 25 };
    private readonly ThemedCheckBox _chkSsl = new() { Text = "Use SSL" };
    private readonly TextBox _txtFrom = new();
    private readonly TextBox _txtTo = new();
    private readonly TextBox _txtSmtpUser = new();
    private readonly TextBox _txtSmtpPass = new() { UseSystemPasswordChar = true };
    private readonly Button _btnTestEmail = new() { Text = "Send Test Email" };
    private readonly Button _btnValidateSmtp = new() { Text = "Validate SMTP" };

    // ── Misc ──
    private readonly Button _btnCopyCmd = new() { Text = "Copy ProcDump Command" };
    private readonly Button _btnSaveConfig = new() { Text = "Save Config" };
    private readonly RichTextBox _txtStatus = new() { ReadOnly = true };
    private readonly Label _lblElevation = new();

    // ── Responsive layout hosts ──
    private SplitContainer _splitMain = null!;
    private Panel _scrollHost = null!;
    private TableLayoutPanel _mainLayout = null!;

    private Config _cfg = new();

    public MainForm()
    {
        Text = "ProcDump Monitor";
        StartPosition = FormStartPosition.CenterScreen;
        MinimumSize = new Size(480, 400);
        Size = new Size(940, 860);
        AutoScaleMode = AutoScaleMode.Dpi;
        Font = new Font("Segoe UI", 9.5f);

        BuildLayout();
        WireEvents();
        LoadConfig();
        CheckElevation();
        AutoDetectProcDump();
        LoadIcon();

        // Apply dark theme last so it covers all controls
        ThemeManager.ApplyTheme(this);

        // Initial task status & action preview
        RefreshTaskStatus();
        RefreshActionPreview();
    }

    // ═══════════════════════════════════════════════════════════════
    //  Icon
    // ═══════════════════════════════════════════════════════════════

    private void LoadIcon()
    {
        try
        {
            // Try runtime path first (next to the exe)
            string icoPath = Path.Combine(AppContext.BaseDirectory, "Assets", "jci_globe.ico");
            if (!File.Exists(icoPath))
            {
                // Try source tree path (for development)
                icoPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Assets", "jci_globe.ico");
            }
            if (File.Exists(icoPath))
                Icon = new Icon(icoPath);
        }
        catch
        {
            // Non-critical — default icon is fine.
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //  Layout
    // ═══════════════════════════════════════════════════════════════

    private void BuildLayout()
    {
        // ── SplitContainer: top = scrollable config, bottom = status log ──
        _splitMain = new SplitContainer
        {
            Dock = DockStyle.Fill,
            Orientation = Orientation.Horizontal,
            FixedPanel = FixedPanel.Panel2,
            SplitterDistance = 600,          // initial; recalculated in Resize
            SplitterWidth = 6,
            Panel1MinSize = 200,
            Panel2MinSize = 120
        };

        // ── Scroll host (Panel1) ──
        _scrollHost = new Panel
        {
            Name = "ScrollHost",
            Dock = DockStyle.Fill,
            AutoScroll = true,
            Padding = new Padding(12)
        };

        // ── Main layout inside scroll host ──
        _mainLayout = new TableLayoutPanel
        {
            Dock = DockStyle.Top,           // Top, not Fill — lets AutoScroll compute height
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 1,
            RowCount = 5,                    // elevation, target, procdump, task, email
            Padding = Padding.Empty,
            Margin = Padding.Empty
        };
        _mainLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        // Row 0 – Elevation warning (AutoSize)
        _mainLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _lblElevation.Dock = DockStyle.Top;
        _lblElevation.AutoSize = true;
        _lblElevation.ForeColor = Color.OrangeRed;
        _lblElevation.Font = new Font(Font, FontStyle.Bold);
        _lblElevation.Margin = new Padding(0, 0, 0, 4);
        _mainLayout.Controls.Add(_lblElevation, 0, 0);

        // Row 1 – Target process (AutoSize)
        _mainLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _mainLayout.Controls.Add(BuildTargetGroup(), 0, 1);

        // Row 2 – ProcDump settings (AutoSize)
        _mainLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _mainLayout.Controls.Add(BuildProcDumpGroup(), 0, 2);

        // Row 3 – Scheduled Task (AutoSize)
        _mainLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _mainLayout.Controls.Add(BuildTaskGroup(), 0, 3);

        // Row 4 – Email (AutoSize)
        _mainLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _mainLayout.Controls.Add(BuildEmailGroup(), 0, 4);

        _scrollHost.Controls.Add(_mainLayout);
        _splitMain.Panel1.Controls.Add(_scrollHost);

        // ── Status log (Panel2) ──
        _splitMain.Panel2.Controls.Add(BuildStatusGroup());

        Controls.Add(_splitMain);

        // ── Resize handler for scroll + button reflow ──
        Resize += OnFormResize;
    }

    // ── Helpers to create AutoSize GroupBoxes with internal TableLayouts ──

    private static GroupBox MakeGroup(string text)
    {
        return new GroupBox
        {
            Text = text,
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            Padding = new Padding(10, 6, 10, 8),
            Margin = new Padding(0, 0, 0, 4)
        };
    }

    private static TableLayoutPanel MakeTable(int cols, int rows)
    {
        var tbl = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = cols,
            RowCount = rows,
            Margin = Padding.Empty
        };
        return tbl;
    }

    private static Label MakeLabel(string text)
    {
        return new Label
        {
            Text = text,
            AutoSize = true,
            Anchor = AnchorStyles.Left,
            Margin = new Padding(0, 6, 6, 0)
        };
    }

    private static FlowLayoutPanel MakeButtonFlow(params Button[] buttons)
    {
        var flow = new FlowLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            WrapContents = true,
            FlowDirection = FlowDirection.LeftToRight,
            Padding = Padding.Empty,
            Margin = new Padding(0, 4, 0, 0)
        };
        foreach (var b in buttons)
        {
            b.AutoSize = true;
            b.AutoSizeMode = AutoSizeMode.GrowAndShrink;
            b.MinimumSize = new Size(80, 30);
            b.Margin = new Padding(0, 0, 6, 4);
            flow.Controls.Add(b);
        }
        return flow;
    }

    // ── Section builders ──

    private GroupBox BuildTargetGroup()
    {
        var grp = MakeGroup("Target Process");
        var tbl = MakeTable(2, 1);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        tbl.Controls.Add(MakeLabel("Target Name:"), 0, 0);
        _txtTarget.Dock = DockStyle.Fill;
        tbl.Controls.Add(_txtTarget, 1, 0);

        grp.Controls.Add(tbl);
        return grp;
    }

    private GroupBox BuildProcDumpGroup()
    {
        var grp = MakeGroup("ProcDump Settings");
        var tbl = MakeTable(3, 8);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));

        int r = 0;
        tbl.Controls.Add(MakeLabel("ProcDump Path:"), 0, r);
        _txtProcDumpPath.Dock = DockStyle.Fill; tbl.Controls.Add(_txtProcDumpPath, 1, r);
        _btnBrowseProcDump.AutoSize = true; tbl.Controls.Add(_btnBrowseProcDump, 2, r);

        r++;
        tbl.Controls.Add(MakeLabel("Dump Directory:"), 0, r);
        _txtDumpDir.Dock = DockStyle.Fill; tbl.Controls.Add(_txtDumpDir, 1, r);
        _btnBrowseDumpDir.AutoSize = true; tbl.Controls.Add(_btnBrowseDumpDir, 2, r);

        r++;
        tbl.Controls.Add(MakeLabel("Dump Type:"), 0, r);
        _cboDumpType.Items.AddRange(new object[] { "Full", "MiniPlus", "Mini" });
        _cboDumpType.SelectedIndex = 0;
        _cboDumpType.Dock = DockStyle.Fill; tbl.Controls.Add(_cboDumpType, 1, r);

        r++;
        _chkException.AutoSize = true; tbl.SetColumnSpan(_chkException, 3);
        tbl.Controls.Add(_chkException, 0, r);

        r++;
        _chkTerminate.AutoSize = true; tbl.SetColumnSpan(_chkTerminate, 3);
        tbl.Controls.Add(_chkTerminate, 0, r);

        r++;
        _chkClone.AutoSize = true; tbl.SetColumnSpan(_chkClone, 3);
        tbl.Controls.Add(_chkClone, 0, r);

        r++;
        var pnlNums = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            WrapContents = true,
            Margin = Padding.Empty
        };
        pnlNums.Controls.Add(MakeLabel("Max dumps:"));
        _nudMaxDumps.Width = 60; pnlNums.Controls.Add(_nudMaxDumps);
        pnlNums.Controls.Add(MakeLabel("Restart delay (s):"));
        _nudRestartDelay.Width = 60; pnlNums.Controls.Add(_nudRestartDelay);
        tbl.SetColumnSpan(pnlNums, 3);
        tbl.Controls.Add(pnlNums, 0, r);

        grp.Controls.Add(tbl);
        return grp;
    }

    private GroupBox BuildTaskGroup()
    {
        var grp = MakeGroup("Scheduled Task");

        // Outer container: banner at top, then content table
        var outer = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 1,
            RowCount = 2,
            Margin = Padding.Empty
        };
        outer.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize)); // banner
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize)); // content

        // ── Banner ──
        BuildBannerPanel();
        outer.Controls.Add(_pnlBanner, 0, 0);

        // ── Content table ──
        var tbl = MakeTable(4, 8);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));

        int r = 0;

        // Row 0 – Task Name
        tbl.Controls.Add(MakeLabel("Task Name:"), 0, r);
        _txtTaskName.Dock = DockStyle.Fill; tbl.SetColumnSpan(_txtTaskName, 3);
        tbl.Controls.Add(_txtTaskName, 1, r);

        // Row 1 – Primary action buttons
        r++;
        var flowActions = MakeButtonFlow(_btnInstall, _btnRunNow, _btnStop, _btnRemove);
        tbl.SetColumnSpan(flowActions, 4);
        tbl.Controls.Add(flowActions, 0, r);

        // Row 2 – Navigation/utility buttons
        r++;
        var flowNav = MakeButtonFlow(_btnOpenDumpFolder, _btnViewLogs, _btnOpenTaskScheduler, _btnViewHistory, _btnEnableHistory);
        tbl.SetColumnSpan(flowNav, 4);
        tbl.Controls.Add(flowNav, 0, r);

        // Row 3 – Task Status header
        r++;
        var lblStatusHeader = MakeLabel("Task Status:");
        lblStatusHeader.Font = new Font(Font, FontStyle.Bold);
        tbl.Controls.Add(lblStatusHeader, 0, r);
        _btnRefreshStatus.AutoSize = true;
        tbl.Controls.Add(_btnRefreshStatus, 1, r);

        // Row 4 – Status fields (left pair)
        r++;
        tbl.Controls.Add(MakeLabel("Exists:"), 0, r);
        tbl.Controls.Add(_lblTaskExists, 1, r);
        tbl.Controls.Add(MakeLabel("State:"), 2, r);
        tbl.Controls.Add(_lblTaskState, 3, r);

        // Row 5 – Status fields (middle pair)
        r++;
        tbl.Controls.Add(MakeLabel("Last Run:"), 0, r);
        tbl.Controls.Add(_lblTaskLastRun, 1, r);
        tbl.Controls.Add(MakeLabel("Result:"), 2, r);
        tbl.Controls.Add(_lblTaskLastResult, 3, r);

        // Row 6 – Next run
        r++;
        tbl.Controls.Add(MakeLabel("Next Run:"), 0, r);
        tbl.Controls.Add(_lblTaskNextRun, 1, r);

        // Row 7 – Action Preview
        r++;
        var previewLabel = MakeLabel("Task Action Preview:");
        tbl.Controls.Add(previewLabel, 0, r);
        var previewPanel = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 2,
            RowCount = 1,
            Margin = Padding.Empty
        };
        previewPanel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        previewPanel.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        _txtActionPreview.Dock = DockStyle.Fill;
        previewPanel.Controls.Add(_txtActionPreview, 0, 0);
        _btnCopyAction.AutoSize = true;
        previewPanel.Controls.Add(_btnCopyAction, 1, 0);
        tbl.SetColumnSpan(previewPanel, 3);
        tbl.Controls.Add(previewPanel, 1, r);

        outer.Controls.Add(tbl, 0, 1);
        grp.Controls.Add(outer);
        return grp;
    }

    private void BuildBannerPanel()
    {
        // Banner layout: [icon/text ........... btn1 btn2 dismiss]
        var bannerFlow = new FlowLayoutPanel
        {
            Dock = DockStyle.Right,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            FlowDirection = FlowDirection.LeftToRight,
            WrapContents = false,
            Margin = Padding.Empty,
            Padding = Padding.Empty
        };

        _btnBannerAction1.Margin = new Padding(4, 0, 4, 0);
        _btnBannerAction2.Margin = new Padding(0, 0, 4, 0);
        _btnBannerDismiss.Margin = new Padding(4, 0, 0, 0);
        _btnBannerDismiss.FlatAppearance.BorderSize = 0;
        _btnBannerDismiss.MinimumSize = new Size(24, 24);

        bannerFlow.Controls.Add(_btnBannerAction1);
        bannerFlow.Controls.Add(_btnBannerAction2);
        bannerFlow.Controls.Add(_btnBannerDismiss);

        _pnlBanner.Controls.Add(bannerFlow);
        _pnlBanner.Controls.Add(_lblBannerText);
    }

    private GroupBox BuildEmailGroup()
    {
        var grp = MakeGroup("Email Notification");
        var tbl = MakeTable(4, 7);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));

        int r = 0;
        _chkEmailEnabled.AutoSize = true; tbl.SetColumnSpan(_chkEmailEnabled, 4);
        tbl.Controls.Add(_chkEmailEnabled, 0, r);

        r++;
        tbl.Controls.Add(MakeLabel("SMTP Server:"), 0, r);
        _txtSmtpServer.Dock = DockStyle.Fill; tbl.Controls.Add(_txtSmtpServer, 1, r);
        tbl.Controls.Add(MakeLabel("Port:"), 2, r);
        _nudSmtpPort.Dock = DockStyle.Fill; tbl.Controls.Add(_nudSmtpPort, 3, r);

        r++;
        _chkSsl.AutoSize = true; tbl.SetColumnSpan(_chkSsl, 4);
        tbl.Controls.Add(_chkSsl, 0, r);

        r++;
        tbl.Controls.Add(MakeLabel("From:"), 0, r);
        _txtFrom.Dock = DockStyle.Fill; tbl.SetColumnSpan(_txtFrom, 3); tbl.Controls.Add(_txtFrom, 1, r);

        r++;
        tbl.Controls.Add(MakeLabel("To:"), 0, r);
        _txtTo.Dock = DockStyle.Fill; tbl.SetColumnSpan(_txtTo, 3); tbl.Controls.Add(_txtTo, 1, r);

        r++;
        tbl.Controls.Add(MakeLabel("SMTP User:"), 0, r);
        _txtSmtpUser.Dock = DockStyle.Fill; tbl.Controls.Add(_txtSmtpUser, 1, r);
        tbl.Controls.Add(MakeLabel("Password:"), 2, r);
        _txtSmtpPass.Dock = DockStyle.Fill; tbl.Controls.Add(_txtSmtpPass, 3, r);

        r++;
        var flow = MakeButtonFlow(_btnTestEmail, _btnValidateSmtp);
        tbl.SetColumnSpan(flow, 4);
        tbl.Controls.Add(flow, 0, r);

        grp.Controls.Add(tbl);
        return grp;
    }

    private GroupBox BuildStatusGroup()
    {
        var grp = new GroupBox
        {
            Text = "Status",
            Dock = DockStyle.Fill,
            Padding = new Padding(10, 6, 10, 8),
            Margin = new Padding(0, 0, 0, 0)
        };

        var innerPanel = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 2
        };
        innerPanel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        // Row 0 – Save/Copy buttons
        innerPanel.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var flow = MakeButtonFlow(_btnSaveConfig, _btnCopyCmd);
        innerPanel.Controls.Add(flow, 0, 0);

        // Row 1 – Status text (fills remaining space)
        innerPanel.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        _txtStatus.Dock = DockStyle.Fill;
        innerPanel.Controls.Add(_txtStatus, 0, 1);

        grp.Controls.Add(innerPanel);
        return grp;
    }

    // ═══════════════════════════════════════════════════════════════
    //  Responsive resize
    // ═══════════════════════════════════════════════════════════════

    private void OnFormResize(object? sender, EventArgs e)
    {
        if (_scrollHost == null || _mainLayout == null) return;

        // Let the scroll host know the content's preferred size so scrollbars
        // appear when the window is too small.
        var pref = _mainLayout.PreferredSize;
        _scrollHost.AutoScrollMinSize = new Size(
            Math.Max(pref.Width + 24, 0),
            Math.Max(pref.Height + 24, 0));
    }

    // ═══════════════════════════════════════════════════════════════
    //  Events
    // ═══════════════════════════════════════════════════════════════

    private void WireEvents()
    {
        _btnBrowseProcDump.Click += (_, _) =>
        {
            using var dlg = new OpenFileDialog
            {
                Title = "Select procdump.exe or procdump64.exe",
                Filter = "ProcDump|procdump*.exe|All EXEs|*.exe",
                CheckFileExists = true
            };
            if (dlg.ShowDialog() == DialogResult.OK)
                _txtProcDumpPath.Text = dlg.FileName;
        };

        _btnBrowseDumpDir.Click += (_, _) =>
        {
            using var dlg = new FolderBrowserDialog { Description = "Select dump output folder" };
            if (dlg.ShowDialog() == DialogResult.OK)
                _txtDumpDir.Text = dlg.SelectedPath;
        };

        // ── Scheduled Task buttons ──

        var toolTip = new ToolTip();
        toolTip.SetToolTip(_btnInstall, "Creates the scheduled task if it does not exist, otherwise updates it with current settings.");
        toolTip.SetToolTip(_btnRefreshStatus, "Re-query the scheduled task and update status fields.");
        toolTip.SetToolTip(_btnOpenTaskScheduler, "Opens the Windows Task Scheduler (taskschd.msc).");
        toolTip.SetToolTip(_btnViewHistory, "Opens Event Viewer on the TaskScheduler operational log.");
        toolTip.SetToolTip(_btnEnableHistory, "Enables the Task Scheduler operational event log.");
        toolTip.SetToolTip(_btnCopyAction, "Copy the task action command to the clipboard.");

        _btnInstall.Click += async (_, _) =>
        {
            SetStatus("Create/Update: working…");
            try
            {
                bool existed = false;
                await Task.Run(() =>
                {
                    ValidateInputs();
                    ApplyUiToConfig();
                    _cfg.Save();
                    existed = TaskSchedulerService.InstallOrUpdate(_cfg);
                });
                string verb = existed ? "updated" : "created";
                ShowBanner(true, $"Scheduled task {verb} successfully.");
                SetStatus($"Create/Update: Scheduled task '{_cfg.TaskName}' {verb}.");
                RefreshTaskStatus();
                RefreshActionPreview();
            }
            catch (ValidationException ex)
            {
                ShowBanner(false, $"Validation error: {ex.Message}");
                SetStatus($"Create/Update: ⚠ {ex.Message}");
            }
            catch (Exception ex)
            {
                ShowBanner(false, $"Failed to create scheduled task: {ex.Message}");
                SetStatus($"Create/Update: ✖ {ex.Message}");
                Logger.Log("TaskUI", $"Create/Update failed: {ex}");
            }
        };

        _btnRunNow.Click += async (_, _) => await RunAsync("Run Task", () =>
        {
            TaskSchedulerService.StartNow(_txtTaskName.Text);
            return $"Task '{_txtTaskName.Text}' started.";
        });

        _btnStop.Click += async (_, _) => await RunAsync("Stop Task", () =>
        {
            TaskSchedulerService.StopTask(_txtTaskName.Text);
            return $"Task '{_txtTaskName.Text}' stopped.";
        });

        _btnRemove.Click += async (_, _) =>
        {
            SetStatus("Remove: working…");
            try
            {
                await Task.Run(() => TaskSchedulerService.RemoveTask(_txtTaskName.Text));
                SetStatus($"Remove: Task '{_txtTaskName.Text}' removed.");
                RefreshTaskStatus();
            }
            catch (Exception ex)
            {
                ShowBanner(false, $"Failed to remove task: {ex.Message}");
                SetStatus($"Remove: ✖ {ex.Message}");
            }
        };

        _btnRefreshStatus.Click += (_, _) => RefreshTaskStatus();

        _btnOpenDumpFolder.Click += (_, _) =>
        {
            string dir = _txtDumpDir.Text;
            if (Directory.Exists(dir))
                Process.Start("explorer.exe", dir);
            else
                SetStatus("Dump directory does not exist yet.");
        };

        _btnViewLogs.Click += (_, _) =>
        {
            string logPath = Logger.LogPath;
            if (File.Exists(logPath))
                Process.Start("notepad.exe", logPath);
            else
                SetStatus("Log file does not exist yet.");
        };

        _btnOpenTaskScheduler.Click += (_, _) =>
        {
            try { Process.Start(new ProcessStartInfo("taskschd.msc") { UseShellExecute = true }); }
            catch (Exception ex) { SetStatus($"Cannot open Task Scheduler: {ex.Message}"); }
        };

        _btnViewHistory.Click += (_, _) =>
        {
            try
            {
                Process.Start(new ProcessStartInfo("eventvwr.msc",
                    "/c:Microsoft-Windows-TaskScheduler/Operational") { UseShellExecute = true });
            }
            catch (Exception ex) { SetStatus($"Cannot open Event Viewer: {ex.Message}"); }
        };

        _btnEnableHistory.Click += async (_, _) => await RunAsync("Enable History", () =>
        {
            var psi = new ProcessStartInfo("wevtutil",
                "set-log Microsoft-Windows-TaskScheduler/Operational /enabled:true")
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardError = true
            };
            using var p = Process.Start(psi)!;
            string err = p.StandardError.ReadToEnd();
            p.WaitForExit(10_000);
            if (p.ExitCode != 0 && !string.IsNullOrWhiteSpace(err))
                throw new InvalidOperationException(err.Trim());
            return "Task Scheduler history log enabled.";
        });

        _btnCopyAction.Click += (_, _) =>
        {
            if (!string.IsNullOrEmpty(_txtActionPreview.Text))
            {
                Clipboard.SetText(_txtActionPreview.Text);
                SetStatus("Task action preview copied to clipboard.");
            }
        };

        // ── Banner buttons ──
        _btnBannerDismiss.Click += (_, _) => HideBanner();
        _btnBannerAction1.Click += (_, _) =>
        {
            // Context-dependent: success → "Open Task" / error → "Copy Error"
            if (_pnlBanner.BackColor == ColorTranslator.FromHtml("#1F6F3A"))
            {
                // Open Task (open Task Scheduler)
                try { Process.Start(new ProcessStartInfo("taskschd.msc") { UseShellExecute = true }); }
                catch { /* best effort */ }
            }
            else
            {
                // Copy error text
                Clipboard.SetText(_lblBannerText.Text);
                SetStatus("Error message copied to clipboard.");
            }
        };
        _btnBannerAction2.Click += (_, _) =>
        {
            if (_pnlBanner.BackColor == ColorTranslator.FromHtml("#1F6F3A"))
            {
                // Run Now
                _btnRunNow.PerformClick();
            }
            else
            {
                // View Scheduler Log
                _btnViewLogs.PerformClick();
            }
        };

        // ── Email ──
        _btnTestEmail.Click += async (_, _) => await RunAsync("Test Email", () =>
        {
            ApplyUiToConfig();
            ValidateEmail();
            EmailNotifier.SendTestEmail(_cfg);
            return "Test email sent successfully.";
        });

        _btnValidateSmtp.Click += async (_, _) => await RunAsync("SMTP Check", () =>
        {
            var (ok, msg) = EmailNotifier.ValidateSmtpConnectivity(_txtSmtpServer.Text, (int)_nudSmtpPort.Value);
            return ok ? $"SMTP OK: {msg}" : $"SMTP FAIL: {msg}";
        });

        // ── Misc ──
        _btnCopyCmd.Click += (_, _) =>
        {
            ApplyUiToConfig();
            string cmd = $"\"{_cfg.ProcDumpPath}\" {_cfg.BuildProcDumpArgs()}";
            Clipboard.SetText(cmd);
            SetStatus($"Copied to clipboard:\n{cmd}");
        };

        _btnSaveConfig.Click += async (_, _) => await RunAsync("Save", () =>
        {
            ApplyUiToConfig();
            _cfg.Save();
            return $"Config saved to {Config.DefaultConfigPath}";
        });

        _lblElevation.Click += (_, _) => RelaunchElevated();
    }

    // ═══════════════════════════════════════════════════════════════
    //  Helpers
    // ═══════════════════════════════════════════════════════════════

    private void LoadConfig()
    {
        _cfg = Config.Load();
        _txtTarget.Text = _cfg.TargetName;
        _txtProcDumpPath.Text = _cfg.ProcDumpPath;
        _txtDumpDir.Text = _cfg.DumpDirectory;
        _cboDumpType.SelectedItem = _cfg.DumpType;
        _chkException.Checked = _cfg.DumpOnException;
        _chkTerminate.Checked = _cfg.DumpOnTerminate;
        _chkClone.Checked = _cfg.UseClone;
        _nudMaxDumps.Value = _cfg.MaxDumps;
        _nudRestartDelay.Value = _cfg.RestartDelaySeconds;
        _txtTaskName.Text = _cfg.TaskName;
        _chkEmailEnabled.Checked = _cfg.EmailEnabled;
        _txtSmtpServer.Text = _cfg.SmtpServer;
        _nudSmtpPort.Value = _cfg.SmtpPort;
        _chkSsl.Checked = _cfg.UseSsl;
        _txtFrom.Text = _cfg.FromAddress;
        _txtTo.Text = _cfg.ToAddress;
        _txtSmtpUser.Text = _cfg.SmtpUsername;
        // Password: only show placeholder if blob exists
        if (!string.IsNullOrEmpty(_cfg.EncryptedPasswordBlob))
            _txtSmtpPass.PlaceholderText = "(stored securely)";
    }

    private void ApplyUiToConfig()
    {
        _cfg.TargetName = _txtTarget.Text.Trim();
        _cfg.ProcDumpPath = _txtProcDumpPath.Text.Trim();
        _cfg.DumpDirectory = _txtDumpDir.Text.Trim();
        _cfg.DumpType = _cboDumpType.SelectedItem?.ToString() ?? "Full";
        _cfg.DumpOnException = _chkException.Checked;
        _cfg.DumpOnTerminate = _chkTerminate.Checked;
        _cfg.UseClone = _chkClone.Checked;
        _cfg.MaxDumps = (int)_nudMaxDumps.Value;
        _cfg.RestartDelaySeconds = (int)_nudRestartDelay.Value;
        _cfg.TaskName = _txtTaskName.Text.Trim();
        _cfg.EmailEnabled = _chkEmailEnabled.Checked;
        _cfg.SmtpServer = _txtSmtpServer.Text.Trim();
        _cfg.SmtpPort = (int)_nudSmtpPort.Value;
        _cfg.UseSsl = _chkSsl.Checked;
        _cfg.FromAddress = _txtFrom.Text.Trim();
        _cfg.ToAddress = _txtTo.Text.Trim();
        _cfg.SmtpUsername = _txtSmtpUser.Text.Trim();

        // Only update password if user typed something new
        string passText = _txtSmtpPass.Text;
        if (!string.IsNullOrEmpty(passText))
            _cfg.SetPassword(passText);
    }

    private void ValidateInputs()
    {
        if (!File.Exists(_txtProcDumpPath.Text.Trim()))
            throw new ValidationException("ProcDump executable not found. Browse to procdump.exe.");

        string dumpDir = _txtDumpDir.Text.Trim();
        if (string.IsNullOrWhiteSpace(dumpDir))
            throw new ValidationException("Dump directory must be specified.");
        if (!Directory.Exists(dumpDir))
        {
            try { Directory.CreateDirectory(dumpDir); }
            catch (Exception ex) { throw new ValidationException($"Cannot create dump directory: {ex.Message}"); }
        }

        if (string.IsNullOrWhiteSpace(_txtTaskName.Text))
            throw new ValidationException("Task name cannot be empty.");
    }

    private void ValidateEmail()
    {
        string from = _txtFrom.Text.Trim();
        string to = _txtTo.Text.Trim();
        if (string.IsNullOrWhiteSpace(from) || !from.Contains('@'))
            throw new ValidationException("From address is not a valid email.");
        if (string.IsNullOrWhiteSpace(to) || !to.Contains('@'))
            throw new ValidationException("To address is not a valid email.");
        if (string.IsNullOrWhiteSpace(_txtSmtpServer.Text))
            throw new ValidationException("SMTP server is required.");
    }

    private void CheckElevation()
    {
        bool elevated;
        using (var identity = WindowsIdentity.GetCurrent())
        {
            var principal = new WindowsPrincipal(identity);
            elevated = principal.IsInRole(WindowsBuiltInRole.Administrator);
        }

        if (!elevated)
        {
            _lblElevation.Text = "⚠ Not running as Administrator – task operations will fail. Click here to relaunch elevated.";
            _lblElevation.Cursor = Cursors.Hand;
        }
        else
        {
            _lblElevation.Text = "";
        }
    }

    private void AutoDetectProcDump()
    {
        if (!string.IsNullOrEmpty(_txtProcDumpPath.Text) && File.Exists(_txtProcDumpPath.Text))
            return;

        string baseDir = AppContext.BaseDirectory;
        // Prefer 64-bit
        string pd64 = Path.Combine(baseDir, "procdump64.exe");
        string pd = Path.Combine(baseDir, "procdump.exe");

        if (File.Exists(pd64))
            _txtProcDumpPath.Text = pd64;
        else if (File.Exists(pd))
            _txtProcDumpPath.Text = pd;
    }

    private static void RelaunchElevated()
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = Environment.ProcessPath ?? "ProcDumpMonitor.exe",
                UseShellExecute = true,
                Verb = "runas"
            };
            Process.Start(psi);
            Application.Exit();
        }
        catch (System.ComponentModel.Win32Exception)
        {
            // User cancelled UAC
        }
    }

    private void SetStatus(string message)
    {
        if (InvokeRequired)
        {
            Invoke(() => SetStatus(message));
            return;
        }
        _txtStatus.Text = $"[{DateTime.Now:HH:mm:ss}] {message}\n{_txtStatus.Text}";
    }

    /// <summary>Run a blocking action on a thread-pool thread and report result/error.</summary>
    private async Task RunAsync(string label, Func<string> action)
    {
        SetStatus($"{label}: working…");
        try
        {
            string result = await Task.Run(action);
            SetStatus($"{label}: {result}");
            RefreshTaskStatus();
        }
        catch (ValidationException ex)
        {
            SetStatus($"{label}: ⚠ {ex.Message}");
            MessageBox.Show(ex.Message, label, MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }
        catch (Exception ex)
        {
            SetStatus($"{label}: ✖ {ex.Message}");
            MessageBox.Show(ex.Message, label, MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    // ── Banner helpers ──────────────────────────────────────

    private void ShowBanner(bool success, string message)
    {
        if (InvokeRequired) { Invoke(() => ShowBanner(success, message)); return; }

        var bgColor = ColorTranslator.FromHtml(success ? "#1F6F3A" : "#8B1E1E");
        _pnlBanner.BackColor = bgColor;
        _lblBannerText.BackColor = bgColor;
        _lblBannerText.ForeColor = Color.White;
        _lblBannerText.Text = message;

        foreach (Control c in new Control[] { _btnBannerAction1, _btnBannerAction2, _btnBannerDismiss })
        {
            c.BackColor = bgColor;
            c.ForeColor = Color.White;
        }

        if (success)
        {
            _btnBannerAction1.Text = "Open Task";
            _btnBannerAction2.Text = "Run Now";
        }
        else
        {
            _btnBannerAction1.Text = "Copy Error";
            _btnBannerAction2.Text = "View Logs";
        }

        _pnlBanner.Visible = true;

        _bannerAutoHideTimer.Stop();
        if (success)
        {
            _bannerAutoHideTimer.Interval = 6000;
            _bannerAutoHideTimer.Tick += BannerAutoHideTick;
            _bannerAutoHideTimer.Start();
        }
    }

    private void HideBanner()
    {
        _bannerAutoHideTimer.Stop();
        _pnlBanner.Visible = false;
    }

    private void BannerAutoHideTick(object? sender, EventArgs e)
    {
        _bannerAutoHideTimer.Stop();
        _bannerAutoHideTimer.Tick -= BannerAutoHideTick;
        HideBanner();
    }

    // ── Task status helpers ──────────────────────────────────

    private void RefreshTaskStatus()
    {
        if (InvokeRequired) { Invoke(RefreshTaskStatus); return; }

        try
        {
            string taskName = _txtTaskName.Text;
            if (string.IsNullOrWhiteSpace(taskName))
            {
                ClearTaskStatus();
                return;
            }

            var info = TaskSchedulerService.GetDetailedStatus(taskName);
            _lblTaskExists.Text  = info.Exists ? "Yes" : "No";
            _lblTaskState.Text   = info.State;
            _lblTaskLastRun.Text  = info.LastRunTime;
            _lblTaskLastResult.Text = info.LastRunResult;
            _lblTaskNextRun.Text  = info.NextRunTime;

            UpdateTaskButtonStates(info.Exists);
        }
        catch
        {
            ClearTaskStatus();
        }
    }

    private void ClearTaskStatus()
    {
        _lblTaskExists.Text = "—";
        _lblTaskState.Text = "—";
        _lblTaskLastRun.Text = "—";
        _lblTaskLastResult.Text = "—";
        _lblTaskNextRun.Text = "—";
        UpdateTaskButtonStates(false);
    }

    private void UpdateTaskButtonStates(bool taskExists)
    {
        _btnRunNow.Enabled  = taskExists;
        _btnStop.Enabled    = taskExists;
        _btnRemove.Enabled  = taskExists;
    }

    private void RefreshActionPreview()
    {
        if (InvokeRequired) { Invoke(RefreshActionPreview); return; }

        try
        {
            ApplyUiToConfig();
            var preview = TaskSchedulerService.BuildActionPreview(_cfg);
            _txtActionPreview.Text = $"\"{preview.ExePath}\" {preview.Arguments}";
            if (!string.IsNullOrEmpty(preview.WorkingDirectory))
                _txtActionPreview.Text += $"\r\nWorkDir: {preview.WorkingDirectory}";
        }
        catch
        {
            _txtActionPreview.Text = string.Empty;
        }
    }
}

internal class ValidationException : Exception
{
    public ValidationException(string message) : base(message) { }
}

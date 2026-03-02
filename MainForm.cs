using System.Diagnostics;
using System.Security.Principal;

namespace ProcDumpMonitor;

public class MainForm : Form
{
    // ── Wizard infrastructure ──
    private readonly TableLayoutPanel _rootLayout = new() { Dock = DockStyle.Fill, ColumnCount = 1, Margin = Padding.Empty, Padding = Padding.Empty };
    private readonly StepIndicator _stepIndicator = new();
    private readonly Panel _pnlHeader = new() { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink };
    private readonly Panel _pnlContent = new() { Name = "WizardContent", Dock = DockStyle.Fill };
    private readonly Panel _pnlFooter = new() { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink };
    private readonly Button _btnBack = new() { Text = "← Back", AutoSize = true, MinimumSize = new Size(90, 34) };
    private readonly Button _btnNext = new() { Text = "Next →", AutoSize = true, MinimumSize = new Size(90, 34) };
    private readonly Label _lblTitle = new() { Text = "ProcDump Monitor", AutoSize = true, Font = new Font("Segoe UI", 14f, FontStyle.Bold) };
    private readonly Label _lblElevation = new() { AutoSize = true, ForeColor = Color.OrangeRed, Cursor = Cursors.Hand };

    // ── Export / Import ──
    private readonly Button _btnExportConfig = new() { Text = "Export…", AutoSize = true, MinimumSize = new Size(80, 34) };
    private readonly Button _btnImportConfig = new() { Text = "Import…", AutoSize = true, MinimumSize = new Size(80, 34) };

    // ── System tray ──
    private readonly NotifyIcon _trayIcon = new();
    private readonly ContextMenuStrip _trayMenu = new();

    // ── State ──
    private Config _cfg = new();
    private WizardPage[] _pages = [];
    private int _currentIndex;

    public MainForm()
    {
        Text = "ProcDump Monitor";
        StartPosition = FormStartPosition.CenterScreen;
        MinimumSize = new Size(620, 520);
        Size = new Size(780, 640);
        AutoScaleMode = AutoScaleMode.Dpi;
        Font = new Font("Segoe UI", 9.5f);

        _cfg = Config.Load();

        BuildLayout();
        WireEvents();
        CheckElevation();
        LoadIcon();

        ThemeManager.ApplyTheme(this);
        ThemeManager.EnableDarkTitleBar(this);

        // Tray icon setup
        _trayIcon.Icon = Icon;
        _trayIcon.Text = "ProcDump Monitor";
        _trayIcon.Visible = false;
        _trayMenu.Items.Add("Open", null, (_, _) => RestoreFromTray());
        _trayMenu.Items.Add("-");
        _trayMenu.Items.Add("Exit", null, (_, _) => { _trayIcon.Visible = false; Application.Exit(); });
        _trayIcon.ContextMenuStrip = _trayMenu;
        _trayIcon.DoubleClick += (_, _) => RestoreFromTray();
        ThemeManager.ApplyTheme(_trayMenu);

        Load += MainForm_Load;
    }

    private void MainForm_Load(object? sender, EventArgs e)
    {
        if (ConfigMigrator.DowngradeWarning)
        {
            MessageBox.Show(
                "This config was created by a newer version of ProcDump Monitor.\n" +
                "Some settings may be ignored.",
                "Config Version Warning",
                MessageBoxButtons.OK,
                MessageBoxIcon.Warning);
        }

        NavigateTo(0);
    }

    // ═══════════════════════════════════════════════════════════════
    //  Icon
    // ═══════════════════════════════════════════════════════════════

    private void LoadIcon()
    {
        try
        {
            using var stream = typeof(MainForm).Assembly
                .GetManifestResourceStream("ProcDumpMonitor.jci_globe.ico");
            if (stream != null)
                Icon = new Icon(stream);
        }
        catch { /* Non-critical */ }
    }

    // ═══════════════════════════════════════════════════════════════
    //  Layout
    // ═══════════════════════════════════════════════════════════════

    private void BuildLayout()
    {
        // ── Header ──
        var headerLayout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 3,
            RowCount = 1,
            Margin = Padding.Empty,
            Padding = new Padding(16, 8, 16, 0)
        };
        headerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        headerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        headerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        _lblTitle.Anchor = AnchorStyles.Left;
        _lblElevation.Anchor = AnchorStyles.Right;
        headerLayout.Controls.Add(_lblTitle, 0, 0);
        headerLayout.Controls.Add(_lblElevation, 2, 0);
        _pnlHeader.Controls.Add(headerLayout);

        // ── Step indicator ──
        _pages =
        [
            new TargetPage(),
            new ProcDumpPage(),
            new TaskPage(),
            new NotificationsPage(),
            new ReviewPage(),
            new AboutPage()
        ];
        _stepIndicator.Configure(_pages.Select(p => p.StepTitle).ToArray());
        _stepIndicator.Padding = new Padding(32, 0, 32, 0);

        // ── Footer ──
        var footerLayout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 5,
            RowCount = 1,
            Padding = new Padding(16, 8, 16, 8)
        };
        footerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); // Export
        footerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); // Import
        footerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); // spacer
        footerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); // Back
        footerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); // Next

        _btnExportConfig.Anchor = AnchorStyles.Left;
        _btnImportConfig.Anchor = AnchorStyles.Left;
        _btnBack.Anchor = AnchorStyles.Right;
        _btnNext.Anchor = AnchorStyles.Right;

        footerLayout.Controls.Add(_btnExportConfig, 0, 0);
        footerLayout.Controls.Add(_btnImportConfig, 1, 0);
        footerLayout.Controls.Add(new Panel(), 2, 0); // spacer
        footerLayout.Controls.Add(_btnBack, 3, 0);
        footerLayout.Controls.Add(_btnNext, 4, 0);
        _pnlFooter.Controls.Add(footerLayout);

        // ── Root layout: 4-row TableLayoutPanel guarantees footer is never pushed off-screen ──
        // Row 0 = Header (AutoSize), Row 1 = StepIndicator (AutoSize),
        // Row 2 = Content (Percent 100 — absorbs all remaining space),
        // Row 3 = Footer (AutoSize — always pinned to bottom).
        _rootLayout.RowCount = 4;
        _rootLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        _rootLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));      // row 0: header
        _rootLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));      // row 1: step indicator
        _rootLayout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));  // row 2: content (fills)
        _rootLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));      // row 3: footer

        _stepIndicator.Dock = DockStyle.Fill;

        _rootLayout.Controls.Add(_pnlHeader, 0, 0);
        _rootLayout.Controls.Add(_stepIndicator, 0, 1);
        _rootLayout.Controls.Add(_pnlContent, 0, 2);
        _rootLayout.Controls.Add(_pnlFooter, 0, 3);

        Controls.Add(_rootLayout);
    }

    // ═══════════════════════════════════════════════════════════════
    //  Navigation
    // ═══════════════════════════════════════════════════════════════

    private void WireEvents()
    {
        _btnBack.Click += (_, _) =>
        {
            if (_currentIndex > 0)
            {
                _pages[_currentIndex].OnLeave(_cfg);
                NavigateTo(_currentIndex - 1);
            }
        };

        _btnNext.Click += (_, _) =>
        {
            var page = _pages[_currentIndex];

            if (!page.IsValid())
            {
                _stepIndicator.MarkError(_currentIndex);
                return;
            }

            if (!page.OnLeave(_cfg))
                return;

            _stepIndicator.MarkCompleted(_currentIndex);

            if (_currentIndex < _pages.Length - 1)
                NavigateTo(_currentIndex + 1);
        };

        _lblElevation.Click += (_, _) => RelaunchElevated();

        _btnExportConfig.Click += (_, _) =>
        {
            using var dlg = new SaveFileDialog
            {
                Title = "Export Config (secrets redacted)",
                Filter = "JSON|*.json",
                DefaultExt = "json",
                FileName = "config_export.json"
            };
            if (dlg.ShowDialog() == DialogResult.OK)
            {
                try
                {
                    CollectAllPages();
                    ConfigExportImport.Export(_cfg, dlg.FileName);
                }
                catch (Exception ex)
                {
                    MessageBox.Show($"Export failed: {ex.Message}", "Export",
                        MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
            }
        };

        _btnImportConfig.Click += (_, _) =>
        {
            using var dlg = new OpenFileDialog
            {
                Title = "Import Config",
                Filter = "JSON|*.json|All|*.*",
                CheckFileExists = true
            };
            if (dlg.ShowDialog() == DialogResult.OK)
            {
                try
                {
                    _cfg = ConfigExportImport.Import(dlg.FileName);
                    // Reset step indicators and restart wizard
                    for (int i = 0; i < _pages.Length; i++)
                        _stepIndicator.ClearMark(i);
                    NavigateTo(0);
                }
                catch (Exception ex)
                {
                    MessageBox.Show($"Import failed: {ex.Message}", "Import",
                        MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
            }
        };
    }

    private void NavigateTo(int index)
    {
        _currentIndex = index;
        var page = _pages[index];

        // Swap content
        _pnlContent.SuspendLayout();
        _pnlContent.Controls.Clear();
        _pnlContent.Controls.Add(page);
        _pnlContent.ResumeLayout(true);

        // Notify page
        page.OnEnter(_cfg);

        // Wire validation changes (remove previous to avoid duplicates)
        page.ValidationChanged -= OnPageValidationChanged;
        page.ValidationChanged += OnPageValidationChanged;

        // Update step indicator
        _stepIndicator.SetCurrentStep(index);

        // Update footer buttons
        _btnBack.Visible = index > 0;

        // On the final step, hide Next — the page has its own action buttons
        bool isFinal = index >= _pages.Length - 1;
        _btnNext.Visible = !isFinal;
        _btnNext.Enabled = page.IsValid();

        // Theme the newly added page
        ThemeManager.ApplyTheme(page);
    }

    private void OnPageValidationChanged(object? sender, EventArgs e)
    {
        var page = _pages[_currentIndex];
        _btnNext.Enabled = page.IsValid();

        if (page.IsValid())
            _stepIndicator.ClearMark(_currentIndex);
    }

    /// <summary>Collect state from all pages into _cfg without validating.</summary>
    private void CollectAllPages()
    {
        foreach (var page in _pages)
            page.OnLeave(_cfg);
    }

    // ═══════════════════════════════════════════════════════════════
    //  Helpers
    // ═══════════════════════════════════════════════════════════════

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
            _lblElevation.Text = "⚠ Not running as Administrator";
            _lblElevation.Font = new Font(Font, FontStyle.Bold);
        }
        else
        {
            _lblElevation.Text = "✓ Administrator";
            _lblElevation.ForeColor = ColorTranslator.FromHtml("#1F6F3A");
            _lblElevation.Font = new Font(Font, FontStyle.Regular);
            _lblElevation.Cursor = Cursors.Default;
        }
    }

    protected override void OnResize(EventArgs e)
    {
        base.OnResize(e);
        if (WindowState == FormWindowState.Minimized)
        {
            _trayIcon.Visible = true;
            ShowInTaskbar = false;
            Hide();
        }
    }

    private void RestoreFromTray()
    {
        Show();
        ShowInTaskbar = true;
        WindowState = FormWindowState.Normal;
        _trayIcon.Visible = false;
        Activate();
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        _trayIcon.Visible = false;
        _trayIcon.Dispose();
        _trayMenu.Dispose();
        base.OnFormClosing(e);
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
}

/// <summary>Item for the service dropdown — displays DisplayName, retains ServiceName.</summary>
internal sealed class ServiceItem
{
    public string ServiceName { get; }
    public string DisplayName { get; }

    public ServiceItem(string serviceName, string displayName)
    {
        ServiceName = serviceName;
        DisplayName = displayName;
    }

    public override string ToString() => DisplayName;
}

internal class ValidationException : Exception
{
    public ValidationException(string message) : base(message) { }
}

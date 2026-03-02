namespace ProcDumpMonitor;

/// <summary>Wizard Step 2 — ProcDump configuration with collapsible advanced triggers.</summary>
public sealed class ProcDumpPage : WizardPage
{
    public override string StepTitle => "ProcDump";

    // ── Required settings ──
    private readonly TextBox _txtProcDumpPath = new();
    private readonly Button _btnBrowseProcDump = new() { Text = "Browse…", AutoSize = true };
    private readonly TextBox _txtDumpDir = new();
    private readonly Button _btnBrowseDumpDir = new() { Text = "Browse…", AutoSize = true };
    private readonly ComboBox _cboDumpType = new() { DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly ThemedCheckBox _chkException = new() { Text = "Dump on unhandled exception (-e)" };
    private readonly ThemedCheckBox _chkTerminate = new() { Text = "Dump on terminate (-t)" };
    private readonly ThemedCheckBox _chkClone = new() { Text = "Use clone (-r)" };
    private readonly NumericUpDown _nudMaxDumps = new() { Minimum = 1, Maximum = 100, Value = 1, Width = 60 };
    private readonly NumericUpDown _nudRestartDelay = new() { Minimum = 1, Maximum = 600, Value = 5, Width = 60 };
    private readonly NumericUpDown _nudMinFreeDiskMB = new() { Minimum = 0, Maximum = 999999, Value = 5120, Width = 90 };

    // ── Advanced triggers (collapsible) ──
    private readonly Button _btnToggleAdvanced = new() { Text = "▶ Advanced Triggers", AutoSize = true, FlatStyle = FlatStyle.Flat, Cursor = Cursors.Hand };
    private readonly Panel _pnlAdvanced = new() { Visible = false, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Top, Margin = Padding.Empty };
    private readonly NumericUpDown _nudCpuThreshold = new() { Minimum = 0, Maximum = 100, Value = 0, Width = 70 };
    private readonly NumericUpDown _nudCpuLowThreshold = new() { Minimum = 0, Maximum = 100, Value = 0, Width = 70 };
    private readonly NumericUpDown _nudMemoryCommitMB = new() { Minimum = 0, Maximum = 999999, Value = 0, Width = 90 };
    private readonly NumericUpDown _nudHangWindowSeconds = new() { Minimum = 0, Maximum = 300, Value = 0, Width = 70 };

    // ── Validation ──
    private readonly Label _lblPathValidation;
    private readonly Label _lblDirValidation;
    private readonly Label _lblTriggerValidation;

    public ProcDumpPage()
    {
        _lblPathValidation = MakeValidationLabel();
        _lblDirValidation = MakeValidationLabel();
        _lblTriggerValidation = MakeValidationLabel();

        _cboDumpType.Items.AddRange(new object[] { "Full", "MiniPlus", "Mini" });
        _cboDumpType.SelectedIndex = 0;

        var outer = new TableLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 1,
            Padding = Padding.Empty,
            Margin = Padding.Empty
        };
        outer.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        int r = 0;

        // ProcDump path
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildPathRow("ProcDump Path:", _txtProcDumpPath, _btnBrowseProcDump), 0, r++);
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_lblPathValidation, 0, r++);

        // Dump directory
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildPathRow("Dump Directory:", _txtDumpDir, _btnBrowseDumpDir), 0, r++);
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_lblDirValidation, 0, r++);

        // Dump type
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var typeRow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, WrapContents = true, Margin = new Padding(0, 8, 0, 0) };
        typeRow.Controls.Add(MakeLabel("Dump Type:"));
        _cboDumpType.Width = 120;
        typeRow.Controls.Add(_cboDumpType);
        outer.Controls.Add(typeRow, 0, r++);

        // Trigger checkboxes
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var triggerGrp = MakeGroup("Triggers");
        var triggerLayout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 1,
            RowCount = 4,
            Margin = Padding.Empty
        };
        triggerLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        _chkException.AutoSize = true;
        _chkTerminate.AutoSize = true;
        _chkClone.AutoSize = true;
        triggerLayout.Controls.Add(_chkException, 0, 0);
        triggerLayout.Controls.Add(_chkTerminate, 0, 1);
        triggerLayout.Controls.Add(_chkClone, 0, 2);
        triggerLayout.Controls.Add(_lblTriggerValidation, 0, 3);
        triggerGrp.Controls.Add(triggerLayout);
        outer.Controls.Add(triggerGrp, 0, r++);

        // Numeric settings
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var numsFlow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, WrapContents = true, Margin = new Padding(0, 8, 0, 0) };
        numsFlow.Controls.Add(MakeLabel("Max dumps:"));
        numsFlow.Controls.Add(_nudMaxDumps);
        numsFlow.Controls.Add(MakeLabel("Restart delay (s):"));
        numsFlow.Controls.Add(_nudRestartDelay);
        numsFlow.Controls.Add(MakeLabel("Min Free Disk (MB):"));
        numsFlow.Controls.Add(_nudMinFreeDiskMB);
        outer.Controls.Add(numsFlow, 0, r++);

        // Advanced triggers expander
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _btnToggleAdvanced.Margin = new Padding(0, 12, 0, 4);
        outer.Controls.Add(_btnToggleAdvanced, 0, r++);

        // Advanced triggers panel
        BuildAdvancedPanel();
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_pnlAdvanced, 0, r++);

        outer.RowCount = r;
        Controls.Add(outer);

        // Events
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

        _btnToggleAdvanced.Click += (_, _) =>
        {
            _pnlAdvanced.Visible = !_pnlAdvanced.Visible;
            _btnToggleAdvanced.Text = _pnlAdvanced.Visible ? "▼ Advanced Triggers" : "▶ Advanced Triggers";
        };

        _txtProcDumpPath.TextChanged += (_, _) => Revalidate();
        _txtDumpDir.TextChanged += (_, _) => Revalidate();
        _chkException.CheckedChanged += (_, _) => Revalidate();
        _chkTerminate.CheckedChanged += (_, _) => Revalidate();
        _nudCpuThreshold.ValueChanged += (_, _) => Revalidate();
        _nudMemoryCommitMB.ValueChanged += (_, _) => Revalidate();
        _nudHangWindowSeconds.ValueChanged += (_, _) => Revalidate();
    }

    private TableLayoutPanel BuildPathRow(string labelText, TextBox txt, Button btn)
    {
        var row = new TableLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 3,
            RowCount = 1,
            Margin = new Padding(0, 6, 0, 0)
        };
        row.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        row.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        row.Controls.Add(MakeLabel(labelText), 0, 0);
        txt.Dock = DockStyle.Fill;
        row.Controls.Add(txt, 1, 0);
        row.Controls.Add(btn, 2, 0);
        return row;
    }

    private void BuildAdvancedPanel()
    {
        var tbl = MakeTable(4, 3);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));

        tbl.Controls.Add(MakeLabel("CPU % (-c):"), 0, 0);
        _nudCpuThreshold.Dock = DockStyle.Left;
        tbl.Controls.Add(_nudCpuThreshold, 1, 0);
        tbl.Controls.Add(MakeLabel("CPU Low % (-cl):"), 2, 0);
        _nudCpuLowThreshold.Dock = DockStyle.Left;
        tbl.Controls.Add(_nudCpuLowThreshold, 3, 0);

        tbl.Controls.Add(MakeLabel("Memory MB (-m):"), 0, 1);
        _nudMemoryCommitMB.Dock = DockStyle.Left;
        tbl.Controls.Add(_nudMemoryCommitMB, 1, 1);
        tbl.Controls.Add(MakeLabel("Hang Window (s):"), 2, 1);
        _nudHangWindowSeconds.Dock = DockStyle.Left;
        tbl.Controls.Add(_nudHangWindowSeconds, 3, 1);

        var warnLabel = new Label
        {
            Text = "⚠ Aggressive thresholds (e.g. CPU ≥ 5%) may produce dumps every few seconds. 0 = disabled.",
            AutoSize = true,
            ForeColor = Color.Orange,
            Margin = new Padding(0, 4, 0, 0)
        };
        tbl.SetColumnSpan(warnLabel, 4);
        tbl.Controls.Add(warnLabel, 0, 2);

        _pnlAdvanced.Controls.Add(tbl);
    }

    private void Revalidate()
    {
        string path = _txtProcDumpPath.Text.Trim();
        bool pathOk = !string.IsNullOrWhiteSpace(path) && File.Exists(path);
        _lblPathValidation.Visible = !string.IsNullOrWhiteSpace(path) && !pathOk;
        _lblPathValidation.Text = "ProcDump executable not found at this path.";

        bool dirOk = !string.IsNullOrWhiteSpace(_txtDumpDir.Text.Trim());
        _lblDirValidation.Visible = !dirOk;
        _lblDirValidation.Text = "Dump directory must be specified.";

        bool hasTrigger = _chkException.Checked || _chkTerminate.Checked ||
                          _nudCpuThreshold.Value > 0 || _nudMemoryCommitMB.Value > 0 ||
                          _nudHangWindowSeconds.Value > 0;
        _lblTriggerValidation.Visible = !hasTrigger;
        _lblTriggerValidation.Text = "At least one trigger must be active.";

        RaiseValidationChanged();
    }

    public override void OnEnter(Config cfg)
    {
        _txtProcDumpPath.Text = cfg.ProcDumpPath;
        _txtDumpDir.Text = cfg.DumpDirectory;
        _cboDumpType.SelectedItem = cfg.DumpType;
        _chkException.Checked = cfg.DumpOnException;
        _chkTerminate.Checked = cfg.DumpOnTerminate;
        _chkClone.Checked = cfg.UseClone;
        _nudMaxDumps.Value = Math.Clamp(cfg.MaxDumps, 1, 100);
        _nudRestartDelay.Value = Math.Clamp(cfg.RestartDelaySeconds, 1, 600);
        _nudMinFreeDiskMB.Value = Math.Clamp(cfg.MinFreeDiskMB, 0, 999999);

        _nudCpuThreshold.Value = Math.Clamp(cfg.CpuThreshold, 0, 100);
        _nudCpuLowThreshold.Value = Math.Clamp(cfg.CpuLowThreshold, 0, 100);
        _nudMemoryCommitMB.Value = Math.Clamp(cfg.MemoryCommitMB, 0, 999999);
        _nudHangWindowSeconds.Value = Math.Clamp(cfg.HangWindowSeconds, 0, 300);

        // Auto-detect ProcDump if not already set
        if (string.IsNullOrEmpty(_txtProcDumpPath.Text) || !File.Exists(_txtProcDumpPath.Text))
        {
            string baseDir = AppContext.BaseDirectory;
            string pd64 = Path.Combine(baseDir, "procdump64.exe");
            string pd = Path.Combine(baseDir, "procdump.exe");
            if (File.Exists(pd64)) _txtProcDumpPath.Text = pd64;
            else if (File.Exists(pd)) _txtProcDumpPath.Text = pd;
        }

        // If any advanced trigger is non-zero, expand the section
        if (cfg.CpuThreshold > 0 || cfg.CpuLowThreshold > 0 || cfg.MemoryCommitMB > 0 || cfg.HangWindowSeconds > 0)
        {
            _pnlAdvanced.Visible = true;
            _btnToggleAdvanced.Text = "▼ Advanced Triggers";
        }

        Revalidate();
    }

    public override bool OnLeave(Config cfg)
    {
        cfg.ProcDumpPath = _txtProcDumpPath.Text.Trim();
        cfg.DumpDirectory = _txtDumpDir.Text.Trim();
        cfg.DumpType = _cboDumpType.SelectedItem?.ToString() ?? "Full";
        cfg.DumpOnException = _chkException.Checked;
        cfg.DumpOnTerminate = _chkTerminate.Checked;
        cfg.UseClone = _chkClone.Checked;
        cfg.MaxDumps = (int)_nudMaxDumps.Value;
        cfg.RestartDelaySeconds = (int)_nudRestartDelay.Value;
        cfg.MinFreeDiskMB = (long)_nudMinFreeDiskMB.Value;

        cfg.CpuThreshold = (int)_nudCpuThreshold.Value;
        cfg.CpuLowThreshold = (int)_nudCpuLowThreshold.Value;
        cfg.MemoryCommitMB = (int)_nudMemoryCommitMB.Value;
        cfg.HangWindowSeconds = (int)_nudHangWindowSeconds.Value;
        return true;
    }

    public override bool IsValid()
    {
        string path = _txtProcDumpPath.Text.Trim();
        bool pathOk = !string.IsNullOrWhiteSpace(path) && File.Exists(path);
        bool dirOk = !string.IsNullOrWhiteSpace(_txtDumpDir.Text.Trim());
        bool hasTrigger = _chkException.Checked || _chkTerminate.Checked ||
                          _nudCpuThreshold.Value > 0 || _nudMemoryCommitMB.Value > 0 ||
                          _nudHangWindowSeconds.Value > 0;
        return pathOk && dirOk && hasTrigger;
    }
}

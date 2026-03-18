namespace ProcDumpMonitor;

/// <summary>Wizard Step 2 — ProcDump configuration with scenario presets,
/// searchable common/advanced options, command preview, and bitness detection.</summary>
public sealed class ProcDumpPage : WizardPage
{
    public override string StepTitle => "ProcDump";

    // ── Scenario ──
    private readonly ComboBox _cboScenario = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 260 };
    private readonly Label _lblScenarioDesc = new() { AutoSize = true, ForeColor = Color.FromArgb(180, 180, 180), MaximumSize = new Size(600, 0), Margin = new Padding(0, 2, 0, 8) };

    // ── Command preview ──
    private readonly TextBox _txtCommandPreview = new() { ReadOnly = true, Dock = DockStyle.Top, BackColor = Color.FromArgb(30, 30, 30), ForeColor = Color.FromArgb(78, 201, 176), Font = new Font("Consolas", 9f) };

    // ── Bitness indicator ──
    private readonly Label _lblBitness = new() { AutoSize = true, ForeColor = Color.FromArgb(200, 200, 100), Margin = new Padding(0, 4, 0, 8) };

    // ── Paths ──
    private readonly TextBox _txtProcDumpPath = new();
    private readonly Button _btnBrowseProcDump = new() { Text = "Browse…", AutoSize = true };
    private readonly TextBox _txtDumpDir = new();
    private readonly Button _btnBrowseDumpDir = new() { Text = "Browse…", AutoSize = true };

    // ── Filter ──
    private readonly TextBox _txtFilterOptions = new() { PlaceholderText = "Filter options…", Dock = DockStyle.Top, Margin = new Padding(0, 8, 0, 4) };

    // ── Common: Dump type ──
    private readonly ComboBox _cboDumpType = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 160 };

    // ── Common: Triggers ──
    private readonly ThemedCheckBox _chkException = new() { Text = "Dump on unhandled exception (-e)", AutoSize = true };
    private readonly ThemedCheckBox _chkHang = new() { Text = "Dump on hung window (-h)", AutoSize = true };
    private readonly ThemedCheckBox _chkTerminate = new() { Text = "Dump on terminate (-t)", AutoSize = true };

    // ── Common: CPU ──
    private readonly NumericUpDown _nudCpuThreshold = new() { Minimum = 0, Maximum = 100, Value = 0, Width = 70 };
    private readonly NumericUpDown _nudCpuLowThreshold = new() { Minimum = 0, Maximum = 100, Value = 0, Width = 70 };
    private readonly NumericUpDown _nudCpuDurationSeconds = new() { Minimum = 0, Maximum = 3600, Value = 0, Width = 70 };
    private readonly NumericUpDown _nudMaxDumps = new() { Minimum = 1, Maximum = 100, Value = 1, Width = 60 };
    private readonly ThemedCheckBox _chkCpuPerUnit = new() { Text = "Per-CPU threshold (-u)", AutoSize = true };

    // ── Common: Memory ──
    private readonly NumericUpDown _nudMemoryCommitMB = new() { Minimum = 0, Maximum = 999999, Value = 0, Width = 90 };

    // ── Common: Operational ──
    private readonly ThemedCheckBox _chkClone = new() { Text = "Use clone / reflect (-r) — minimises process suspension", AutoSize = true };
    private readonly ThemedCheckBox _chkAvoidOutage = new() { Text = "Avoid outage (-a) — exit if triggers fire too rapidly", AutoSize = true };
    private readonly ThemedCheckBox _chkOverwrite = new() { Text = "Overwrite existing dump files (-o)", AutoSize = true };
    private readonly ThemedCheckBox _chkWaitForProcess = new() { Text = "Wait for process to launch (-w)", AutoSize = true, Checked = true };
    private readonly ThemedCheckBox _chkAcceptEula = new() { Text = "Accept EULA (-accepteula) — always required", AutoSize = true, Checked = true, Enabled = false };

    // ── Common: Numeric settings ──
    private readonly NumericUpDown _nudRestartDelay = new() { Minimum = 1, Maximum = 600, Value = 5, Width = 60 };
    private readonly NumericUpDown _nudMinFreeDiskMB = new() { Minimum = 0, Maximum = 999999, Value = 5120, Width = 90 };

    // ── Advanced: Performance Counter ──
    private readonly TextBox _txtPerfCounter = new() { Width = 300, PlaceholderText = @"\Category\Counter\Threshold" };
    private readonly TextBox _txtPerfCounterThreshold = new() { Width = 300, PlaceholderText = @"\Category\Counter\Threshold" };

    // ── Advanced: Exception Filter ──
    private readonly TextBox _txtExFilterInclude = new() { Width = 300, PlaceholderText = "e.g. OutOfMemory,StackOverflow" };
    private readonly TextBox _txtExFilterExclude = new() { Width = 300, PlaceholderText = "e.g. ThreadAbort" };

    // ── Advanced: WER + Timeout ──
    private readonly ThemedCheckBox _chkWer = new() { Text = "Register as WER debugger (-wer)", AutoSize = true };
    private readonly NumericUpDown _nudAvoidTerminateTimeout = new() { Minimum = 0, Maximum = 3600, Value = 0, Width = 70 };

    // ── Advanced panel toggle ──
    private readonly Button _btnToggleAdvanced = new() { Text = "▶ Advanced Options — most users should leave these empty", AutoSize = true, FlatStyle = FlatStyle.Flat, Cursor = Cursors.Hand };
    private readonly Panel _pnlAdvanced = new() { Visible = false, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Top, Margin = Padding.Empty };

    // ── Validation ──
    private readonly Label _lblPathValidation;
    private readonly Label _lblDirValidation;
    private readonly Label _lblWarnings;

    // ── Searchable groups ──
    private readonly List<GroupBox> _optionGroups = new();

    // ── State ──
    private bool _suppressPresetSwitch;

    public ProcDumpPage()
    {
        _lblPathValidation = MakeValidationLabel();
        _lblDirValidation = MakeValidationLabel();
        _lblWarnings = new Label { AutoSize = true, ForeColor = Color.Orange, Visible = false, Margin = new Padding(0, 4, 0, 0), MaximumSize = new Size(650, 0) };

        _cboDumpType.Items.AddRange(new object[] { "Full", "MiniPlus", "Mini", "ThreadDump" });
        _cboDumpType.SelectedIndex = 0;

        foreach (var preset in ProcDumpPreset.Preset.All)
            _cboScenario.Items.Add(preset.Name);
        _cboScenario.Items.Add("Custom");
        _cboScenario.SelectedIndex = 0;

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

        // ── Scenario ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var scenarioRow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, WrapContents = true };
        scenarioRow.Controls.Add(MakeLabel("Scenario:"));
        scenarioRow.Controls.Add(_cboScenario);
        outer.Controls.Add(scenarioRow, 0, r++);
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_lblScenarioDesc, 0, r++);

        // ── Guidance ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var lblGuidance = new Label
        {
            Text = "\u2139\uFE0F  Your selected scenario pre-configures all the options below. Most users do not need to change anything on this page.",
            AutoSize = true,
            ForeColor = Color.FromArgb(140, 180, 220),
            Font = new Font("Segoe UI", 8.5f, FontStyle.Italic),
            MaximumSize = new Size(620, 0),
            Margin = new Padding(0, 2, 0, 8)
        };
        outer.Controls.Add(lblGuidance, 0, r++);

        // ── Command preview ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var previewRow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Margin = new Padding(0, 0, 0, 4) };
        previewRow.Controls.Add(MakeLabel("Effective command:"));
        outer.Controls.Add(previewRow, 0, r++);
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_txtCommandPreview, 0, r++);

        // ── Bitness ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _lblBitness.Text = "Target bitness: detecting…";
        outer.Controls.Add(_lblBitness, 0, r++);

        // ── Paths ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildPathRow("ProcDump Path:", _txtProcDumpPath, _btnBrowseProcDump), 0, r++);
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_lblPathValidation, 0, r++);

        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildPathRow("Dump Directory:", _txtDumpDir, _btnBrowseDumpDir), 0, r++);
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_lblDirValidation, 0, r++);

        // ── Filter ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_txtFilterOptions, 0, r++);

        // ── Common groups ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildDumpTypeGroup(), 0, r++);

        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildTriggersGroup(), 0, r++);

        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildCpuGroup(), 0, r++);

        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildMemoryGroup(), 0, r++);

        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildOperationalGroup(), 0, r++);

        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(BuildNumericSettingsFlow(), 0, r++);

        // ── Warnings ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_lblWarnings, 0, r++);

        // ── Advanced toggle ──
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _btnToggleAdvanced.Margin = new Padding(0, 12, 0, 4);
        outer.Controls.Add(_btnToggleAdvanced, 0, r++);

        // ── Advanced panel ──
        BuildAdvancedPanel();
        outer.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        outer.Controls.Add(_pnlAdvanced, 0, r++);

        outer.RowCount = r;
        Controls.Add(outer);

        WireEvents();
    }

    // ═══════════════════════════════════════════════════════════════
    //  Group builders
    // ═══════════════════════════════════════════════════════════════

    private GroupBox BuildDumpTypeGroup()
    {
        var grp = MakeGroup("Dump Type");
        grp.Tag = "dump type full miniplus mini thread -ma -mp -mm -mt";
        var flow = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink };
        flow.Controls.Add(MakeLabel("Type:"));
        flow.Controls.Add(_cboDumpType);
        var hint = new Label { Text = "Full (-ma) = all memory · MiniPlus (-mp) = private memory · Mini (-mm) = stacks only · Thread (-mt) = text stacks", AutoSize = true, ForeColor = Color.Gray, MaximumSize = new Size(500, 0), Margin = new Padding(8, 6, 0, 0) };
        flow.Controls.Add(hint);
        grp.Controls.Add(flow);
        _optionGroups.Add(grp);
        return grp;
    }

    private GroupBox BuildTriggersGroup()
    {
        var grp = MakeGroup("Triggers");
        grp.Tag = "trigger exception hang terminate crash -e -h -t";
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, ColumnCount = 1, RowCount = 3, Margin = Padding.Empty };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.Controls.Add(_chkException, 0, 0);
        layout.Controls.Add(_chkHang, 0, 1);
        layout.Controls.Add(_chkTerminate, 0, 2);
        grp.Controls.Add(layout);
        _optionGroups.Add(grp);
        return grp;
    }

    private GroupBox BuildCpuGroup()
    {
        var grp = MakeGroup("CPU Options");
        grp.Tag = "cpu threshold duration seconds count per-cpu -c -cl -s -n -u";
        var tbl = MakeTable(4, 3);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));

        tbl.Controls.Add(MakeLabel("CPU % (-c):"), 0, 0);
        tbl.Controls.Add(_nudCpuThreshold, 1, 0);
        tbl.Controls.Add(MakeLabel("CPU Low % (-cl):"), 2, 0);
        tbl.Controls.Add(_nudCpuLowThreshold, 3, 0);

        tbl.Controls.Add(MakeLabel("Duration sec (-s):"), 0, 1);
        tbl.Controls.Add(_nudCpuDurationSeconds, 1, 1);
        tbl.Controls.Add(MakeLabel("Count (-n):"), 2, 1);
        tbl.Controls.Add(_nudMaxDumps, 3, 1);

        tbl.Controls.Add(_chkCpuPerUnit, 0, 2);
        tbl.SetColumnSpan(_chkCpuPerUnit, 4);

        grp.Controls.Add(tbl);
        _optionGroups.Add(grp);
        return grp;
    }

    private GroupBox BuildMemoryGroup()
    {
        var grp = MakeGroup("Memory");
        grp.Tag = "memory commit threshold megabyte -m";
        var flow = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink };
        flow.Controls.Add(MakeLabel("Commit threshold MB (-m):"));
        flow.Controls.Add(_nudMemoryCommitMB);
        var hint = new Label { Text = "0 = disabled. Set above normal working set to catch leaks.", AutoSize = true, ForeColor = Color.Gray, Margin = new Padding(8, 6, 0, 0) };
        flow.Controls.Add(hint);
        grp.Controls.Add(flow);
        _optionGroups.Add(grp);
        return grp;
    }

    private GroupBox BuildOperationalGroup()
    {
        var grp = MakeGroup("Operational");
        grp.Tag = "operational clone reflect avoid outage overwrite wait process eula -r -a -o -w -accepteula";
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, ColumnCount = 1, RowCount = 5, Margin = Padding.Empty };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.Controls.Add(_chkClone, 0, 0);
        layout.Controls.Add(_chkAvoidOutage, 0, 1);
        layout.Controls.Add(_chkOverwrite, 0, 2);
        layout.Controls.Add(_chkWaitForProcess, 0, 3);
        layout.Controls.Add(_chkAcceptEula, 0, 4);
        grp.Controls.Add(layout);
        _optionGroups.Add(grp);
        return grp;
    }

    private FlowLayoutPanel BuildNumericSettingsFlow()
    {
        var flow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, WrapContents = true, Margin = new Padding(0, 8, 0, 0) };
        flow.Controls.Add(MakeLabel("Restart delay (s):"));
        flow.Controls.Add(_nudRestartDelay);
        flow.Controls.Add(MakeLabel("Min Free Disk (MB):"));
        flow.Controls.Add(_nudMinFreeDiskMB);
        return flow;
    }

    private void BuildAdvancedPanel()
    {
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 1, Margin = Padding.Empty, Padding = Padding.Empty
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        int r = 0;

        // Performance counter
        var perfGrp = MakeGroup("Performance Counters");
        perfGrp.Tag = "performance counter threshold -p -pl";
        var perfTbl = MakeTable(2, 3);
        perfTbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        perfTbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var perfHelp = new Label
        {
            Text = "When to use: Only for custom counter-based triggers (e.g., handle count, I/O rate).\nMost users should leave this blank.",
            AutoSize = true, ForeColor = Color.Gray, MaximumSize = new Size(560, 0), Margin = new Padding(0, 0, 0, 6)
        };
        perfTbl.Controls.Add(perfHelp, 0, 0);
        perfTbl.SetColumnSpan(perfHelp, 2);
        perfTbl.Controls.Add(MakeLabel("Above threshold (-p):"), 0, 1);
        _txtPerfCounter.Dock = DockStyle.Fill;
        perfTbl.Controls.Add(_txtPerfCounter, 1, 1);
        perfTbl.Controls.Add(MakeLabel("Below threshold (-pl):"), 0, 2);
        _txtPerfCounterThreshold.Dock = DockStyle.Fill;
        perfTbl.Controls.Add(_txtPerfCounterThreshold, 1, 2);
        perfGrp.Controls.Add(perfTbl);
        _optionGroups.Add(perfGrp);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(perfGrp, 0, r++);

        // Exception filter
        var exGrp = MakeGroup("Exception Filtering");
        exGrp.Tag = "exception filter include exclude -f -fx";
        var exTbl = MakeTable(2, 3);
        exTbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        exTbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var exHelp = new Label
        {
            Text = "When to use: Only for narrowing specific exception types.\nCrash Capture already handles unhandled exceptions automatically. Most users should leave this blank.",
            AutoSize = true, ForeColor = Color.Gray, MaximumSize = new Size(560, 0), Margin = new Padding(0, 0, 0, 6)
        };
        exTbl.Controls.Add(exHelp, 0, 0);
        exTbl.SetColumnSpan(exHelp, 2);
        exTbl.Controls.Add(MakeLabel("Include filter (-f):"), 0, 1);
        _txtExFilterInclude.Dock = DockStyle.Fill;
        exTbl.Controls.Add(_txtExFilterInclude, 1, 1);
        exTbl.Controls.Add(MakeLabel("Exclude filter (-fx):"), 0, 2);
        _txtExFilterExclude.Dock = DockStyle.Fill;
        exTbl.Controls.Add(_txtExFilterExclude, 1, 2);
        exGrp.Controls.Add(exTbl);
        _optionGroups.Add(exGrp);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(exGrp, 0, r++);

        // WER + Timeout
        var werGrp = MakeGroup("WER & Timeout");
        werGrp.Tag = "wer windows error reporting timeout avoid terminate -wer -at";
        var werLayout = new TableLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, ColumnCount = 2, RowCount = 4, Margin = Padding.Empty };
        werLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        werLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        var werHelp = new Label
        {
            Text = "When to use: WER integration registers ProcDump as the Windows Error Reporting debugger. Only needed when the standard exception trigger (-e) is not capturing crashes because WER handles them first. Most users do not need this.",
            AutoSize = true, ForeColor = Color.Gray, MaximumSize = new Size(560, 0), Margin = new Padding(0, 0, 0, 6)
        };
        werLayout.Controls.Add(werHelp, 0, 0);
        werLayout.SetColumnSpan(werHelp, 2);
        werLayout.Controls.Add(_chkWer, 0, 1);
        werLayout.SetColumnSpan(_chkWer, 2);
        var atHelp = new Label
        {
            Text = "Avoid-terminate timeout is for rare edge cases where ProcDump takes too long to write a dump, blocking process shutdown. Leave at 0 unless directed by support.",
            AutoSize = true, ForeColor = Color.Gray, MaximumSize = new Size(560, 0), Margin = new Padding(0, 4, 0, 4)
        };
        werLayout.Controls.Add(atHelp, 0, 2);
        werLayout.SetColumnSpan(atHelp, 2);
        werLayout.Controls.Add(MakeLabel("Avoid-terminate timeout (-at, seconds):"), 0, 3);
        werLayout.Controls.Add(_nudAvoidTerminateTimeout, 1, 3);
        werGrp.Controls.Add(werLayout);
        _optionGroups.Add(werGrp);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(werGrp, 0, r++);

        layout.RowCount = r;
        _pnlAdvanced.Controls.Add(layout);
    }

    // ═══════════════════════════════════════════════════════════════
    //  Path row helper
    // ═══════════════════════════════════════════════════════════════

    private TableLayoutPanel BuildPathRow(string labelText, TextBox txt, Button btn)
    {
        var row = new TableLayoutPanel
        {
            Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 3, RowCount = 1, Margin = new Padding(0, 6, 0, 0)
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

        _btnToggleAdvanced.Click += (_, _) =>
        {
            _pnlAdvanced.Visible = !_pnlAdvanced.Visible;
            _btnToggleAdvanced.Text = _pnlAdvanced.Visible
                ? "▼ Advanced Options — most users should leave these empty"
                : "▶ Advanced Options — most users should leave these empty";
        };

        _cboScenario.SelectedIndexChanged += (_, _) =>
        {
            if (_suppressPresetSwitch) return;
            string selected = _cboScenario.SelectedItem?.ToString() ?? "Custom";
            if (selected == "Custom")
            {
                _lblScenarioDesc.Text = "Configure ProcDump options manually.";
                return;
            }

            var preset = ProcDumpPreset.Preset.FindByName(selected);
            if (preset == null) return;

            _lblScenarioDesc.Text = preset.Description;
            ApplyPreset(preset);
        };

        _txtFilterOptions.TextChanged += (_, _) => ApplyFilter();

        EventHandler optionChanged = (_, _) => OnOptionChanged();
        _cboDumpType.SelectedIndexChanged += optionChanged;
        _chkException.CheckedChanged += optionChanged;
        _chkHang.CheckedChanged += optionChanged;
        _chkTerminate.CheckedChanged += optionChanged;
        _chkClone.CheckedChanged += optionChanged;
        _chkAvoidOutage.CheckedChanged += optionChanged;
        _chkOverwrite.CheckedChanged += optionChanged;
        _chkWaitForProcess.CheckedChanged += optionChanged;
        _chkCpuPerUnit.CheckedChanged += optionChanged;
        _chkWer.CheckedChanged += optionChanged;

        _nudCpuThreshold.ValueChanged += optionChanged;
        _nudCpuLowThreshold.ValueChanged += optionChanged;
        _nudCpuDurationSeconds.ValueChanged += optionChanged;
        _nudMaxDumps.ValueChanged += optionChanged;
        _nudMemoryCommitMB.ValueChanged += optionChanged;
        _nudRestartDelay.ValueChanged += optionChanged;
        _nudMinFreeDiskMB.ValueChanged += optionChanged;
        _nudAvoidTerminateTimeout.ValueChanged += optionChanged;

        _txtPerfCounter.TextChanged += optionChanged;
        _txtPerfCounterThreshold.TextChanged += optionChanged;
        _txtExFilterInclude.TextChanged += optionChanged;
        _txtExFilterExclude.TextChanged += optionChanged;

        _txtProcDumpPath.TextChanged += (_, _) => { UpdatePreview(); Revalidate(); };
        _txtDumpDir.TextChanged += (_, _) => { UpdatePreview(); Revalidate(); };
    }

    private void OnOptionChanged()
    {
        if (!_suppressPresetSwitch && _cboScenario.SelectedItem?.ToString() != "Custom")
        {
            _suppressPresetSwitch = true;
            _cboScenario.SelectedIndex = _cboScenario.Items.Count - 1;
            _lblScenarioDesc.Text = "Configure ProcDump options manually.";
            _suppressPresetSwitch = false;
        }
        UpdatePreview();
        Revalidate();
    }

    // ═══════════════════════════════════════════════════════════════
    //  Preset application
    // ═══════════════════════════════════════════════════════════════

    private void ApplyPreset(ProcDumpPreset.Preset preset)
    {
        _suppressPresetSwitch = true;
        try
        {
            var tempCfg = new Config
            {
                TargetName = "target",
                DumpDirectory = _txtDumpDir.Text.Trim(),
                ProcDumpPath = _txtProcDumpPath.Text.Trim(),
                WaitForProcess = _chkWaitForProcess.Checked
            };
            preset.Apply(tempCfg);
            PopulateControlsFromConfig(tempCfg);
        }
        finally
        {
            _suppressPresetSwitch = false;
        }
        UpdatePreview();
        Revalidate();
    }

    private void PopulateControlsFromConfig(Config cfg)
    {
        _cboDumpType.SelectedItem = cfg.DumpType;
        _chkException.Checked = cfg.DumpOnException;
        _chkHang.Checked = cfg.HangWindowSeconds > 0;
        _chkTerminate.Checked = cfg.DumpOnTerminate;
        _chkClone.Checked = cfg.UseClone;
        _chkAvoidOutage.Checked = cfg.AvoidOutage;
        _chkOverwrite.Checked = cfg.OverwriteExisting;
        _chkCpuPerUnit.Checked = cfg.CpuPerUnit;
        _nudCpuThreshold.Value = Math.Clamp(cfg.CpuThreshold, 0, 100);
        _nudCpuLowThreshold.Value = Math.Clamp(cfg.CpuLowThreshold, 0, 100);
        _nudCpuDurationSeconds.Value = Math.Clamp(cfg.CpuDurationSeconds, 0, 3600);
        _nudMaxDumps.Value = Math.Clamp(cfg.MaxDumps, 1, 100);
        _nudMemoryCommitMB.Value = Math.Clamp(cfg.MemoryCommitMB, 0, 999999);
        _chkWer.Checked = cfg.WerIntegration;
        _nudAvoidTerminateTimeout.Value = Math.Clamp(cfg.AvoidTerminateTimeout, 0, 3600);
        _txtPerfCounter.Text = cfg.PerformanceCounter;
        _txtPerfCounterThreshold.Text = cfg.PerfCounterThreshold;
        _txtExFilterInclude.Text = cfg.ExceptionFilterInclude;
        _txtExFilterExclude.Text = cfg.ExceptionFilterExclude;
    }

    // ═══════════════════════════════════════════════════════════════
    //  Filter
    // ═══════════════════════════════════════════════════════════════

    private void ApplyFilter()
    {
        string filter = _txtFilterOptions.Text.Trim();
        foreach (var grp in _optionGroups)
        {
            if (grp.Tag is string terms)
                grp.Visible = string.IsNullOrEmpty(filter) || terms.Contains(filter, StringComparison.OrdinalIgnoreCase);
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //  Command preview
    // ═══════════════════════════════════════════════════════════════

    private void UpdatePreview()
    {
        var tempCfg = BuildTempConfig();
        _txtCommandPreview.Text = $"\"{tempCfg.ProcDumpPath}\" {tempCfg.BuildProcDumpArgs()}";
    }

    private Config BuildTempConfig()
    {
        return new Config
        {
            ProcDumpPath = _txtProcDumpPath.Text.Trim(),
            DumpDirectory = _txtDumpDir.Text.Trim(),
            TargetName = "(target)",
            DumpType = _cboDumpType.SelectedItem?.ToString() ?? "Full",
            DumpOnException = _chkException.Checked,
            DumpOnTerminate = _chkTerminate.Checked,
            HangWindowSeconds = _chkHang.Checked ? 1 : 0,
            UseClone = _chkClone.Checked,
            AvoidOutage = _chkAvoidOutage.Checked,
            OverwriteExisting = _chkOverwrite.Checked,
            WaitForProcess = _chkWaitForProcess.Checked,
            CpuPerUnit = _chkCpuPerUnit.Checked,
            CpuThreshold = (int)_nudCpuThreshold.Value,
            CpuLowThreshold = (int)_nudCpuLowThreshold.Value,
            CpuDurationSeconds = (int)_nudCpuDurationSeconds.Value,
            MaxDumps = (int)_nudMaxDumps.Value,
            MemoryCommitMB = (int)_nudMemoryCommitMB.Value,
            WerIntegration = _chkWer.Checked,
            AvoidTerminateTimeout = (int)_nudAvoidTerminateTimeout.Value,
            PerformanceCounter = _txtPerfCounter.Text.Trim(),
            PerfCounterThreshold = _txtPerfCounterThreshold.Text.Trim(),
            ExceptionFilterInclude = _txtExFilterInclude.Text.Trim(),
            ExceptionFilterExclude = _txtExFilterExclude.Text.Trim(),
        };
    }

    // ═══════════════════════════════════════════════════════════════
    //  Bitness detection
    // ═══════════════════════════════════════════════════════════════

    private void DetectBitnessAsync(string targetName)
    {
        _lblBitness.Text = "Target bitness: detecting…";
        _lblBitness.ForeColor = Color.FromArgb(200, 200, 100);

        if (string.IsNullOrWhiteSpace(targetName))
        {
            _lblBitness.Text = "Target bitness: no target specified";
            return;
        }

        Task.Run(() =>
        {
            try
            {
                var result = ProcDumpBitnessResolver.Resolve(targetName, AppPaths.InstallDir);
                if (IsDisposed) return;
                Invoke(() =>
                {
                    _lblBitness.Text = $"Target bitness: {result.Summary}";
                    _lblBitness.ForeColor = result.Warning != null
                        ? Color.Orange
                        : Color.FromArgb(100, 200, 100);

                    if (!string.IsNullOrEmpty(result.ActualBinary) && File.Exists(result.ActualBinary))
                        _txtProcDumpPath.Text = result.ActualBinary;

                    if (result.Warning != null)
                        Logger.Log("Bitness", result.Warning);
                });
            }
            catch (Exception ex)
            {
                if (IsDisposed) return;
                Invoke(() =>
                {
                    _lblBitness.Text = $"Target bitness: detection failed ({ex.Message})";
                    _lblBitness.ForeColor = Color.Orange;
                });
            }
        });
    }

    // ═══════════════════════════════════════════════════════════════
    //  Validation
    // ═══════════════════════════════════════════════════════════════

    private void Revalidate()
    {
        string path = _txtProcDumpPath.Text.Trim();
        bool pathOk = !string.IsNullOrWhiteSpace(path) && File.Exists(path);
        _lblPathValidation.Visible = !string.IsNullOrWhiteSpace(path) && !pathOk;
        _lblPathValidation.Text = "ProcDump executable not found at this path.";

        bool dirOk = !string.IsNullOrWhiteSpace(_txtDumpDir.Text.Trim());
        _lblDirValidation.Visible = !dirOk;
        _lblDirValidation.Text = "Dump directory must be specified.";

        var tempCfg = BuildTempConfig();
        var result = ProcDumpOptionsValidator.Validate(tempCfg);
        if (result.Warnings.Count > 0)
        {
            _lblWarnings.Text = "⚠ " + string.Join(" · ", result.Warnings);
            _lblWarnings.Visible = true;
        }
        else
        {
            _lblWarnings.Visible = false;
        }

        RaiseValidationChanged();
    }

    // ═══════════════════════════════════════════════════════════════
    //  WizardPage overrides
    // ═══════════════════════════════════════════════════════════════

    public override void OnEnter(Config cfg)
    {
        _suppressPresetSwitch = true;
        try
        {
            _txtProcDumpPath.Text = cfg.ProcDumpPath;
            _txtDumpDir.Text = cfg.DumpDirectory;
            _chkWaitForProcess.Checked = cfg.WaitForProcess;
            _nudRestartDelay.Value = Math.Clamp(cfg.RestartDelaySeconds, 1, 600);
            _nudMinFreeDiskMB.Value = Math.Clamp(cfg.MinFreeDiskMB, 0, 999999);

            PopulateControlsFromConfig(cfg);

            if (!string.IsNullOrEmpty(cfg.Scenario))
            {
                int idx = _cboScenario.Items.IndexOf(cfg.Scenario);
                // Known preset → select it; unknown name → default to Crash capture (index 0), never Custom
                _cboScenario.SelectedIndex = idx >= 0 ? idx : 0;
            }
            else
            {
                // No scenario data (legacy/new config) → default to Crash capture
                _cboScenario.SelectedIndex = 0;
            }

            if (_cboScenario.SelectedItem?.ToString() == "Custom")
                _lblScenarioDesc.Text = "Configure ProcDump options manually.";
            else
            {
                var preset = ProcDumpPreset.Preset.FindByName(_cboScenario.SelectedItem?.ToString() ?? "");
                _lblScenarioDesc.Text = preset?.Description ?? "";
            }

            // Auto-detect ProcDump if not set
            if (string.IsNullOrEmpty(_txtProcDumpPath.Text) || !File.Exists(_txtProcDumpPath.Text))
            {
                string baseDir = AppPaths.InstallDir;
                string pd64 = Path.Combine(baseDir, "procdump64.exe");
                string pd = Path.Combine(baseDir, "procdump.exe");
                if (File.Exists(pd64)) _txtProcDumpPath.Text = pd64;
                else if (File.Exists(pd)) _txtProcDumpPath.Text = pd;
            }

            // Expand advanced if any advanced option is set
            if (!string.IsNullOrWhiteSpace(cfg.PerformanceCounter) ||
                !string.IsNullOrWhiteSpace(cfg.PerfCounterThreshold) ||
                !string.IsNullOrWhiteSpace(cfg.ExceptionFilterInclude) ||
                !string.IsNullOrWhiteSpace(cfg.ExceptionFilterExclude) ||
                cfg.WerIntegration || cfg.AvoidTerminateTimeout > 0)
            {
                _pnlAdvanced.Visible = true;
                _btnToggleAdvanced.Text = "▼ Advanced Options — most users should leave these empty";
            }
        }
        finally
        {
            _suppressPresetSwitch = false;
        }

        UpdatePreview();
        Revalidate();

        if (!string.IsNullOrWhiteSpace(cfg.TargetName))
            DetectBitnessAsync(cfg.TargetName);
        else
            _lblBitness.Text = "Target bitness: configure target on previous page";
    }

    public override bool OnLeave(Config cfg)
    {
        cfg.ProcDumpPath = _txtProcDumpPath.Text.Trim();
        cfg.DumpDirectory = _txtDumpDir.Text.Trim();
        cfg.DumpType = _cboDumpType.SelectedItem?.ToString() ?? "Full";
        cfg.DumpOnException = _chkException.Checked;
        cfg.DumpOnTerminate = _chkTerminate.Checked;
        cfg.HangWindowSeconds = _chkHang.Checked ? 1 : 0;
        cfg.UseClone = _chkClone.Checked;
        cfg.AvoidOutage = _chkAvoidOutage.Checked;
        cfg.OverwriteExisting = _chkOverwrite.Checked;
        cfg.WaitForProcess = _chkWaitForProcess.Checked;
        cfg.CpuPerUnit = _chkCpuPerUnit.Checked;
        cfg.CpuThreshold = (int)_nudCpuThreshold.Value;
        cfg.CpuLowThreshold = (int)_nudCpuLowThreshold.Value;
        cfg.CpuDurationSeconds = (int)_nudCpuDurationSeconds.Value;
        cfg.MaxDumps = (int)_nudMaxDumps.Value;
        cfg.MemoryCommitMB = (int)_nudMemoryCommitMB.Value;
        cfg.RestartDelaySeconds = (int)_nudRestartDelay.Value;
        cfg.MinFreeDiskMB = (long)_nudMinFreeDiskMB.Value;
        cfg.WerIntegration = _chkWer.Checked;
        cfg.AvoidTerminateTimeout = (int)_nudAvoidTerminateTimeout.Value;
        cfg.PerformanceCounter = _txtPerfCounter.Text.Trim();
        cfg.PerfCounterThreshold = _txtPerfCounterThreshold.Text.Trim();
        cfg.ExceptionFilterInclude = _txtExFilterInclude.Text.Trim();
        cfg.ExceptionFilterExclude = _txtExFilterExclude.Text.Trim();

        string scenarioName = _cboScenario.SelectedItem?.ToString() ?? "Custom";
        cfg.Scenario = scenarioName == "Custom" ? "" : scenarioName;

        return true;
    }

    public override bool IsValid()
    {
        string path = _txtProcDumpPath.Text.Trim();
        bool pathOk = !string.IsNullOrWhiteSpace(path) && File.Exists(path);
        bool dirOk = !string.IsNullOrWhiteSpace(_txtDumpDir.Text.Trim());

        var tempCfg = BuildTempConfig();
        var result = ProcDumpOptionsValidator.Validate(tempCfg);

        // Only block on errors unrelated to path/dir (we check those separately above)
        bool validatorOk = result.Errors.All(e =>
            !e.Contains("ProcDump executable") && !e.Contains("Dump directory"));

        return pathOk && dirOk && validatorOk;
    }
}

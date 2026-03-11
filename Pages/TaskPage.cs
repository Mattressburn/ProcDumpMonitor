namespace ProcDumpMonitor;

/// <summary>Wizard Step 3 — Scheduled Task name and existing task detection.</summary>
public sealed class TaskPage : WizardPage
{
    public override string StepTitle => "Task";

    private readonly TextBox _txtTaskName = new() { Dock = DockStyle.Top };
    private readonly Button _btnResetAuto = new() { Text = "Reset to Auto", AutoSize = true, MinimumSize = new Size(100, 28) };
    private readonly Label _lblValidation;

    // Detection badge
    private readonly Panel _pnlBadge = new()
    {
        Dock = DockStyle.Top,
        Height = 48,
        Padding = new Padding(12, 10, 12, 10),
        Margin = new Padding(0, 12, 0, 8)
    };
    private readonly Label _lblBadge = new() { AutoSize = true, ForeColor = Color.White, Font = new Font("Segoe UI", 10f, FontStyle.Bold) };

    // Task details (shown when task exists)
    private readonly Label _lblState = new() { AutoSize = true };
    private readonly Label _lblLastRun = new() { AutoSize = true };
    private readonly Label _lblLastResult = new() { AutoSize = true };
    private readonly Label _lblNextRun = new() { AutoSize = true };
    private readonly GroupBox _grpDetails;

    // Action preview
    private readonly TextBox _txtActionPreview = new() { ReadOnly = true, Multiline = true, ScrollBars = ScrollBars.Vertical, Height = 52, Dock = DockStyle.Fill };
    private readonly Button _btnCopyAction = new() { Text = "Copy Command", AutoSize = true };

    private bool _taskExists;

    /// <summary>Tracks whether the user has manually edited the task name.
    /// When true, auto-population from the target name is suppressed.</summary>
    private bool _userEditedTaskName;

    /// <summary>The last auto-generated task name, used to detect user edits.</summary>
    private string _lastAutoName = "";

    public TaskPage()
    {
        _lblValidation = MakeValidationLabel();
        _lblValidation.Text = "Task name cannot be empty.";

        // Details group
        _grpDetails = MakeGroup("Existing Task Details");
        var detailTbl = MakeTable(4, 2);
        detailTbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        detailTbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        detailTbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        detailTbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        detailTbl.Controls.Add(MakeLabel("State:"), 0, 0);
        detailTbl.Controls.Add(_lblState, 1, 0);
        detailTbl.Controls.Add(MakeLabel("Last Run:"), 2, 0);
        detailTbl.Controls.Add(_lblLastRun, 3, 0);
        detailTbl.Controls.Add(MakeLabel("Result:"), 0, 1);
        detailTbl.Controls.Add(_lblLastResult, 1, 1);
        detailTbl.Controls.Add(MakeLabel("Next Run:"), 2, 1);
        detailTbl.Controls.Add(_lblNextRun, 3, 1);
        _grpDetails.Controls.Add(detailTbl);
        _grpDetails.Visible = false;

        // Action preview group
        var previewGrp = MakeGroup("Task Action Preview");
        var previewLayout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 2,
            RowCount = 1,
            Margin = Padding.Empty
        };
        previewLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        previewLayout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        previewLayout.Controls.Add(_txtActionPreview, 0, 0);
        previewLayout.Controls.Add(_btnCopyAction, 1, 0);
        previewGrp.Controls.Add(previewLayout);

        // Main layout
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 1,
            Padding = Padding.Empty,
            Margin = Padding.Empty
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        int r = 0;
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(MakeLabel("Scheduled Task Name"), 0, r++);

        // Task name + Reset to Auto button in a flow row
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var nameRow = new TableLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 2,
            RowCount = 1,
            Margin = Padding.Empty
        };
        nameRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        nameRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        _txtTaskName.Dock = DockStyle.Fill;
        nameRow.Controls.Add(_txtTaskName, 0, 0);
        nameRow.Controls.Add(_btnResetAuto, 1, 0);
        layout.Controls.Add(nameRow, 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblValidation, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_pnlBadge, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_grpDetails, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(previewGrp, 0, r++);

        layout.RowCount = r;
        Controls.Add(layout);

        _pnlBadge.Controls.Add(_lblBadge);

        // Events
        _txtTaskName.TextChanged += (_, _) =>
        {
            _lblValidation.Visible = string.IsNullOrWhiteSpace(_txtTaskName.Text);

            // Detect manual edit: if user types something different from auto-name, flag it
            if (!string.IsNullOrEmpty(_lastAutoName) &&
                _txtTaskName.Text.Trim() != _lastAutoName)
            {
                _userEditedTaskName = true;
            }

            RaiseValidationChanged();
        };

        _btnResetAuto.Click += (_, _) =>
        {
            _userEditedTaskName = false;
            if (!string.IsNullOrEmpty(_lastAutoName))
                _txtTaskName.Text = _lastAutoName;
        };

        _btnCopyAction.Click += (_, _) =>
        {
            if (!string.IsNullOrEmpty(_txtActionPreview.Text))
            {
                try
                {
                    Clipboard.SetText(_txtActionPreview.Text);
                }
                catch (Exception ex)
                {
                    _lblValidation.Text = $"Clipboard error: {ex.Message}";
                    _lblValidation.Visible = true;
                }
            }
        };
    }

    public override void OnEnter(Config cfg)
    {
        // Compute the auto-generated name from the target
        string autoName = !string.IsNullOrWhiteSpace(cfg.TargetName)
            ? TaskNameHelper.Sanitize($"ProcDump Monitor - {cfg.TargetName}")
            : "";
        _lastAutoName = autoName;

        // Determine if the current task name was auto-generated or user-provided
        string currentTaskName = cfg.TaskName?.Trim() ?? "";
        bool isDefaultName = string.IsNullOrWhiteSpace(currentTaskName) ||
                             currentTaskName == "ProcDump Monitor";

        if (!_userEditedTaskName && !string.IsNullOrEmpty(autoName) && isDefaultName)
        {
            // Auto-populate from target
            cfg.TaskName = autoName;
        }
        else if (!_userEditedTaskName && !string.IsNullOrEmpty(autoName) &&
                 IsAutoGeneratedName(currentTaskName))
        {
            // Previously auto-generated → update to match new target
            cfg.TaskName = autoName;
        }

        _txtTaskName.Text = cfg.TaskName;

        // Detect existing task
        DetectExistingTask(cfg.TaskName);

        // Build action preview
        try
        {
            var preview = TaskSchedulerService.BuildActionPreview(cfg);
            _txtActionPreview.Text = $"\"{preview.ExePath}\" {preview.Arguments}";
            if (!string.IsNullOrEmpty(preview.WorkingDirectory))
                _txtActionPreview.Text += $"\r\nWorkDir: {preview.WorkingDirectory}";
        }
        catch
        {
            _txtActionPreview.Text = string.Empty;
        }
    }

    public override bool OnLeave(Config cfg)
    {
        cfg.TaskName = TaskNameHelper.Sanitize(_txtTaskName.Text.Trim());
        return true;
    }

    public override bool IsValid() => !string.IsNullOrWhiteSpace(_txtTaskName.Text);

    /// <summary>Whether the task already exists (used by ReviewPage to pick Create vs Update).</summary>
    public bool TaskExists => _taskExists;

    /// <summary>Returns true if the name matches the "ProcDump Monitor - xxx" auto-generated pattern.</summary>
    private static bool IsAutoGeneratedName(string name) =>
        name.StartsWith("ProcDump Monitor", StringComparison.OrdinalIgnoreCase);

    private void DetectExistingTask(string taskName)
    {
        try
        {
            if (string.IsNullOrWhiteSpace(taskName))
            {
                SetBadge("querying", "Enter a task name above.");
                _grpDetails.Visible = false;
                _taskExists = false;
                return;
            }

            var info = TaskSchedulerService.GetDetailedStatus(taskName);
            _taskExists = info.Exists;

            if (info.Exists)
            {
                SetBadge("exists", $"✓ Task '{taskName}' exists — will be updated");
                _lblState.Text = info.State;
                _lblLastRun.Text = info.LastRunTime;
                _lblLastResult.Text = info.LastRunResult;
                _lblNextRun.Text = info.NextRunTime;
                _grpDetails.Visible = true;
            }
            else
            {
                SetBadge("new", $"○ No existing task — will be created");
                _grpDetails.Visible = false;
            }
        }
        catch (Exception ex)
        {
            SetBadge("error", $"✖ Cannot query task scheduler: {ex.Message}");
            _grpDetails.Visible = false;
            _taskExists = false;
        }
    }

    private void SetBadge(string state, string message)
    {
        _pnlBadge.BackColor = state switch
        {
            "exists" => ColorTranslator.FromHtml("#1F6F3A"),
            "new" => ColorTranslator.FromHtml("#3C3C3C"),
            "error" => ColorTranslator.FromHtml("#8B1E1E"),
            _ => ColorTranslator.FromHtml("#2D2D30")
        };
        _lblBadge.Text = message;
        _lblBadge.ForeColor = Color.White;
        _lblBadge.BackColor = _pnlBadge.BackColor;
    }
}

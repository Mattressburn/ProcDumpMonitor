using System.Diagnostics;
using System.ServiceProcess;
using System.IO;

namespace ProcDumpMonitor;

/// <summary>Wizard Step 1 -- Target process selection.</summary>
public sealed class TargetPage : WizardPage
{
    public override string StepTitle => "Target";

    private readonly Label _lblInstruction = new()
    {
        Text = "Select a target type, then choose a running process or Windows service. Full process image names are shown for accuracy.",
        AutoSize = true,
        Margin = new Padding(0, 0, 0, 16)
    };

    private readonly RadioButton _rbProcess = new() { Text = "Process", AutoSize = true, Checked = true, Margin = new Padding(0, 0, 12, 0) };
    private readonly RadioButton _rbService = new() { Text = "Service", AutoSize = true };
    private readonly ComboBox _cboProcess = new() { Dock = DockStyle.Top, DropDownStyle = ComboBoxStyle.DropDownList, DropDownWidth = 400 };
    private readonly ComboBox _cboService = new() { Dock = DockStyle.Top, DropDownStyle = ComboBoxStyle.DropDownList, DropDownWidth = 400 };
    private readonly Label _lblProcessInfo = new() { AutoSize = true, ForeColor = Color.Gray, Visible = false, Margin = new Padding(0, 4, 0, 0) };
    private readonly Label _lblServiceInfo;
    private readonly Label _lblProcessHeader;
    private readonly Label _lblServiceHeader;
    private readonly Label _lblValidation;
    private readonly Button _btnRefresh = new() { Text = "Refresh List", AutoSize = true, Margin = new Padding(0, 8, 0, 0) };
    private readonly ThemedCheckBox _chkShowAll = new() { Text = "Show all services (including stopped)", AutoSize = true, Margin = new Padding(0, 4, 0, 0) };

    private bool _suppressComboEvents = false;

    public TargetPage()
    {
        _lblValidation = MakeValidationLabel();
        _lblValidation.Text = "Target selection is required.";
        _lblServiceInfo = new Label
        {
            AutoSize = true,
            ForeColor = Color.Gray,
            Visible = false,
            Margin = new Padding(0, 4, 0, 0)
        };

        // Explicit section headers so visibility can be toggled reliably
        _lblProcessHeader = MakeLabel("Select Process (image name)");
        _lblServiceHeader = MakeLabel("Select Service");

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
        layout.Controls.Add(_lblInstruction, 0, r++);

        // Target type selector
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var typeFlow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, WrapContents = false };
        typeFlow.Controls.Add(_rbProcess);
        typeFlow.Controls.Add(_rbService);
        layout.Controls.Add(typeFlow, 0, r++);

        // Process selection
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblProcessHeader, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_cboProcess, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblProcessInfo, 0, r++);

        // Service selection
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblServiceHeader, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_cboService, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblServiceInfo, 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var btnFlow = new FlowLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            WrapContents = true,
            FlowDirection = FlowDirection.LeftToRight,
            Margin = new Padding(0, 4, 0, 0),
            Padding = Padding.Empty
        };
        btnFlow.Controls.Add(_btnRefresh);
        btnFlow.Controls.Add(_chkShowAll);
        layout.Controls.Add(btnFlow, 0, r++);

        layout.RowCount = r;
        Controls.Add(layout);

        _btnRefresh.Click += (_, _) => RefreshTargetLists();
        _chkShowAll.CheckedChanged += (_, _) => PopulateServices();

        _rbProcess.CheckedChanged += (_, _) => UpdateTargetTypeUI();
        _rbService.CheckedChanged += (_, _) => UpdateTargetTypeUI();

        _cboProcess.SelectedIndexChanged += (_, _) =>
        {
            if (_suppressComboEvents) return;
            if (_cboProcess.SelectedItem is ProcessItem pi && pi.PID > 0)
            {
                _lblProcessInfo.Text = $"Process: {pi.FullImageName}.exe (PID: {pi.PID})";
                _lblProcessInfo.Visible = true;
            }
            else
            {
                _lblProcessInfo.Visible = false;
            }
            RaiseValidationChanged();
        };
        _cboService.SelectedIndexChanged += (_, _) =>
        {
            if (_suppressComboEvents) return;
            if (_cboService.SelectedItem is ServiceItem si && !string.IsNullOrEmpty(si.ServiceName))
            {
                _lblServiceInfo.Text = $"Service: {si.DisplayName} ({si.ServiceName})";
                _lblServiceInfo.Visible = true;
            }
            else
            {
                _lblServiceInfo.Visible = false;
            }
            RaiseValidationChanged();
        };

        UpdateTargetTypeUI();
    }

    private void UpdateTargetTypeUI()
    {
        bool processMode = _rbProcess.Checked;

        // Process controls
        _lblProcessHeader.Visible = processMode;
        _cboProcess.Visible = processMode;
        _lblProcessInfo.Visible = processMode && (_cboProcess.SelectedItem is ProcessItem pi && pi.PID > 0);

        // Service controls
        _lblServiceHeader.Visible = !processMode;
        _cboService.Visible = !processMode;
        _lblServiceInfo.Visible = !processMode && (_cboService.SelectedItem is ServiceItem si && !string.IsNullOrEmpty(si.ServiceName));
        _chkShowAll.Visible = !processMode;
    }

    // Refresh button uses same safe repopulation flow as OnEnter
    private void RefreshTargetLists()
    {
        _suppressComboEvents = true;
        try
        {
            PopulateServices();
            PopulateProcesses();
        }
        finally
        {
            _suppressComboEvents = false;
        }
        UpdateTargetTypeUI();
    }

    public override void OnEnter(Config cfg)
    {
        _suppressComboEvents = true;
        try
        {
            PopulateServices();
            PopulateProcesses();
            // Pre-select from config
            if (cfg.TargetType == TargetType.Service)
            {
                _rbService.Checked = true;
                for (int i = 0; i < _cboService.Items.Count; i++)
                {
                    if (_cboService.Items[i] is ServiceItem si && si.ServiceName.Equals(cfg.TargetName, StringComparison.OrdinalIgnoreCase))
                    {
                        _cboService.SelectedIndex = i;
                        break;
                    }
                }
            }
            else // Default to process
            {
                _rbProcess.Checked = true;
                for (int i = 0; i < _cboProcess.Items.Count; i++)
                {
                    if (_cboProcess.Items[i] is ProcessItem pi && pi.FullImageName.Equals(cfg.TargetName, StringComparison.OrdinalIgnoreCase))
                    {
                        _cboProcess.SelectedIndex = i;
                        break;
                    }
                }
            }
        }
        finally
        {
            _suppressComboEvents = false;
        }
        UpdateTargetTypeUI();
    }

    public override bool OnLeave(Config cfg)
    {
        if (_rbProcess.Checked && _cboProcess.SelectedItem is ProcessItem pi && pi.PID > 0)
        {
            cfg.TargetName = pi.FullImageName;
            cfg.TargetType = TargetType.Process;
        }
        else if (_rbService.Checked && _cboService.SelectedItem is ServiceItem si && !string.IsNullOrEmpty(si.ServiceName))
        {
            cfg.TargetName = si.ServiceName;
            cfg.TargetType = TargetType.Service;
        }
        else
        {
            cfg.TargetName = string.Empty;
        }
        return true;
    }

    public override bool IsValid() =>
        (_rbProcess.Checked && _cboProcess.SelectedItem is ProcessItem pi && pi.PID > 0) ||
        (_rbService.Checked && _cboService.SelectedItem is ServiceItem si && !string.IsNullOrEmpty(si.ServiceName));

    private void PopulateProcesses()
    {
        _suppressComboEvents = true;
        try
        {
            var previous = _cboProcess.SelectedItem as ProcessItem;
            _cboProcess.Items.Clear();
            _cboProcess.Items.Add(new ProcessItem(0, "(none)", ""));
            var processes = Process.GetProcesses()
                .OrderByDescending(p => p.ProcessName.StartsWith("SoftwareHouse.", StringComparison.OrdinalIgnoreCase))
                .ThenBy(p => p.ProcessName, StringComparer.OrdinalIgnoreCase)
                .Select(p =>
                {
                    string imageName = string.Empty;
                    try { imageName = Path.GetFileNameWithoutExtension(p.MainModule?.ModuleName ?? p.ProcessName); } catch { imageName = p.ProcessName; }
                    return new ProcessItem(p.Id, imageName, p.ProcessName);
                })
                .GroupBy(p => p.FullImageName)
                .Select(g => g.First())
                .ToList();
            foreach (var pi in processes)
                _cboProcess.Items.Add(pi);
            int idx = 0;
            if (previous != null && previous.PID > 0)
            {
                for (int i = 1; i < _cboProcess.Items.Count; i++)
                {
                    if (_cboProcess.Items[i] is ProcessItem pi && pi.PID == previous.PID)
                    { idx = i; break; }
                }
            }
            _cboProcess.SelectedIndex = idx;
        }
        finally
        {
            _suppressComboEvents = false;
        }
    }

    private void PopulateServices()
    {
        _suppressComboEvents = true;
        ServiceController[]? controllers = null;
        try
        {
            var previous = _cboService.SelectedItem as ServiceItem;
            _cboService.Items.Clear();
            _cboService.Items.Add(new ServiceItem("", "(none)"));
            controllers = ServiceController.GetServices();
            var filtered = _chkShowAll.Checked
                ? controllers.OrderBy(s => s.DisplayName, StringComparer.OrdinalIgnoreCase)
                : controllers.Where(s => s.Status == ServiceControllerStatus.Running).OrderBy(s => s.DisplayName, StringComparer.OrdinalIgnoreCase);
            foreach (var svc in filtered)
            {
                string suffix = _chkShowAll.Checked ? $" [{svc.Status}]" : "";
                _cboService.Items.Add(new ServiceItem(svc.ServiceName, $"{svc.DisplayName}{suffix}"));
            }
            int idx = 0;
            if (previous != null && !string.IsNullOrEmpty(previous.ServiceName))
            {
                for (int i = 1; i < _cboService.Items.Count; i++)
                {
                    if (_cboService.Items[i] is ServiceItem si && si.ServiceName.Equals(previous.ServiceName, StringComparison.OrdinalIgnoreCase))
                    { idx = i; break; }
                }
            }
            _cboService.SelectedIndex = idx;
        }
        catch (Exception ex)
        {
            _lblServiceInfo.Text = $"Could not enumerate services: {ex.Message}";
            _lblServiceInfo.Visible = true;
        }
        finally
        {
            if (controllers != null)
                foreach (var sc in controllers)
                    sc.Dispose();
            _suppressComboEvents = false;
        }
    }

    private record ProcessItem(int PID, string FullImageName, string ProcessName)
    {
        public override string ToString() => $"{FullImageName} (PID: {PID})";
    }
    private record ServiceItem(string ServiceName, string DisplayName)
    {
        public override string ToString() => $"{DisplayName} [{ServiceName}]";
    }
}

// Extension for label visibility (for concise toggling)
internal static class ControlExtensions
{
    public static void SetVisible(this Control ctrl, bool visible)
    {
        if (ctrl != null) ctrl.Visible = visible;
    }
}

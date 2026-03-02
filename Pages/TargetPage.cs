using System.ServiceProcess;

namespace ProcDumpMonitor;

/// <summary>Wizard Step 1 — Target process selection.</summary>
public sealed class TargetPage : WizardPage
{
    public override string StepTitle => "Target";

    private readonly Label _lblInstruction = new()
    {
        Text = "Enter the process name to monitor, or select a running Windows service.",
        AutoSize = true,
        Margin = new Padding(0, 0, 0, 16)
    };

    private readonly TextBox _txtTarget = new() { Dock = DockStyle.Top, PlaceholderText = "e.g. SoftwareHouse.CrossFire.Server" };
    private readonly Label _lblValidation;

    private readonly Label _lblDivider = new()
    {
        Text = "— OR select a running service —",
        AutoSize = true,
        ForeColor = Color.Gray,
        TextAlign = ContentAlignment.MiddleCenter,
        Dock = DockStyle.Top,
        Margin = new Padding(0, 20, 0, 8)
    };

    private readonly ComboBox _cboService = new() { Dock = DockStyle.Top, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly Button _btnRefresh = new() { Text = "↻ Refresh Services", AutoSize = true, Margin = new Padding(0, 8, 0, 0) };

    public TargetPage()
    {
        _lblValidation = MakeValidationLabel();
        _lblValidation.Text = "Process name is required.";

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

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(MakeLabel("Process Name"), 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_txtTarget, 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblValidation, 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblDivider, 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(MakeLabel("Select Running Service"), 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_cboService, 0, r++);

        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_btnRefresh, 0, r++);

        layout.RowCount = r;
        Controls.Add(layout);

        _txtTarget.TextChanged += (_, _) =>
        {
            _lblValidation.Visible = string.IsNullOrWhiteSpace(_txtTarget.Text);
            RaiseValidationChanged();
        };

        _cboService.SelectedIndexChanged += (_, _) =>
        {
            if (_cboService.SelectedItem is ServiceItem svc && !string.IsNullOrEmpty(svc.ServiceName))
                _txtTarget.Text = svc.ServiceName;
        };

        _btnRefresh.Click += (_, _) => PopulateServices();
    }

    public override void OnEnter(Config cfg)
    {
        _txtTarget.Text = cfg.TargetName;
        PopulateServices();
        _txtTarget.Focus();
    }

    public override bool OnLeave(Config cfg)
    {
        cfg.TargetName = _txtTarget.Text.Trim();
        return true;
    }

    public override bool IsValid() => !string.IsNullOrWhiteSpace(_txtTarget.Text);

    private void PopulateServices()
    {
        ServiceController[]? controllers = null;
        try
        {
            var previous = _cboService.SelectedItem as ServiceItem;
            _cboService.Items.Clear();
            _cboService.Items.Add(new ServiceItem("", "(none — type name manually)"));

            controllers = ServiceController.GetServices();
            var services = controllers
                .Where(s => s.Status == ServiceControllerStatus.Running)
                .OrderBy(s => s.DisplayName, StringComparer.OrdinalIgnoreCase);

            foreach (var svc in services)
                _cboService.Items.Add(new ServiceItem(svc.ServiceName, svc.DisplayName));

            int idx = 0;
            if (previous != null && !string.IsNullOrEmpty(previous.ServiceName))
            {
                for (int i = 1; i < _cboService.Items.Count; i++)
                {
                    if (_cboService.Items[i] is ServiceItem si &&
                        si.ServiceName.Equals(previous.ServiceName, StringComparison.OrdinalIgnoreCase))
                    { idx = i; break; }
                }
            }
            _cboService.SelectedIndex = idx;
        }
        catch { /* Best-effort; service enumeration may fail without admin. */ }
        finally
        {
            if (controllers != null)
                foreach (var sc in controllers)
                    sc.Dispose();
        }
    }
}

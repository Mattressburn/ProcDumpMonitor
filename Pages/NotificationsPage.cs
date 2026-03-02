namespace ProcDumpMonitor;

/// <summary>Wizard Step 4 — Email and webhook notification configuration.</summary>
public sealed class NotificationsPage : WizardPage
{
    public override string StepTitle => "Notify";

    // ── Email ──
    private readonly ThemedCheckBox _chkEmailEnabled = new() { Text = "Enable email notifications", AutoSize = true };
    private readonly Panel _pnlEmail = new() { AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Top, Margin = Padding.Empty };
    private readonly TextBox _txtSmtpServer = new();
    private readonly NumericUpDown _nudSmtpPort = new() { Minimum = 1, Maximum = 65535, Value = 25 };
    private readonly ThemedCheckBox _chkSsl = new() { Text = "Use SSL", AutoSize = true };
    private readonly TextBox _txtFrom = new();
    private readonly TextBox _txtTo = new();
    private readonly TextBox _txtCc = new();
    private readonly TextBox _txtSmtpUser = new();
    private readonly TextBox _txtSmtpPass = new() { UseSystemPasswordChar = true };
    private readonly Button _btnTestEmail = new() { Text = "Send Test Email" };
    private readonly Button _btnValidateSmtp = new() { Text = "Validate SMTP" };
    private readonly Label _lblEmailResult;

    // ── Webhook ──
    private readonly ThemedCheckBox _chkWebhookEnabled = new() { Text = "Enable webhook notifications", AutoSize = true };
    private readonly Panel _pnlWebhook = new() { AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Top, Margin = Padding.Empty };
    private readonly TextBox _txtWebhookUrl = new();

    // ── Maintenance (folded into this step since it's optional settings) ──
    private readonly Button _btnToggleMaintenance = new() { Text = "▶ Maintenance & Retention", AutoSize = true, FlatStyle = FlatStyle.Flat, Cursor = Cursors.Hand };
    private readonly Panel _pnlMaintenance = new() { Visible = false, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Top, Margin = Padding.Empty };
    private readonly NumericUpDown _nudMaxLogSizeMB = new() { Minimum = 1, Maximum = 1000, Value = 10, Width = 70 };
    private readonly NumericUpDown _nudMaxLogFiles = new() { Minimum = 1, Maximum = 100, Value = 5, Width = 60 };
    private readonly NumericUpDown _nudRetentionDays = new() { Minimum = 0, Maximum = 3650, Value = 0, Width = 70 };
    private readonly NumericUpDown _nudRetentionMaxGB = new() { Minimum = 0, Maximum = 10000, Value = 0, DecimalPlaces = 1, Width = 80 };
    private readonly NumericUpDown _nudStabilityTimeout = new() { Minimum = 5, Maximum = 300, Value = 30, Width = 70 };

    // ── Validation ──
    private readonly Label _lblEmailValidation;

    private Config _cfg = new();

    public NotificationsPage()
    {
        _lblEmailResult = MakeValidationLabel();
        _lblEmailValidation = MakeValidationLabel();

        BuildEmailPanel();
        BuildWebhookPanel();
        BuildMaintenancePanel();

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

        // Email section
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_chkEmailEnabled, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_pnlEmail, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_lblEmailValidation, 0, r++);

        // Spacer
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 16));
        layout.Controls.Add(new Panel { Height = 16 }, 0, r++);

        // Webhook section
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_chkWebhookEnabled, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_pnlWebhook, 0, r++);

        // Maintenance expander
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        _btnToggleMaintenance.Margin = new Padding(0, 16, 0, 4);
        layout.Controls.Add(_btnToggleMaintenance, 0, r++);
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(_pnlMaintenance, 0, r++);

        layout.RowCount = r;
        Controls.Add(layout);

        // Events
        _chkEmailEnabled.CheckedChanged += (_, _) =>
        {
            _pnlEmail.Visible = _chkEmailEnabled.Checked;
            _lblEmailValidation.Visible = false;
            RaiseValidationChanged();
        };

        _chkWebhookEnabled.CheckedChanged += (_, _) =>
        {
            _pnlWebhook.Visible = _chkWebhookEnabled.Checked;
        };

        _btnToggleMaintenance.Click += (_, _) =>
        {
            _pnlMaintenance.Visible = !_pnlMaintenance.Visible;
            _btnToggleMaintenance.Text = _pnlMaintenance.Visible ? "▼ Maintenance & Retention" : "▶ Maintenance & Retention";
        };

        _btnTestEmail.Click += async (_, _) =>
        {
            _lblEmailResult.ForeColor = Color.Gray;
            _lblEmailResult.Text = "Sending test email…";
            _lblEmailResult.Visible = true;
            try
            {
                ApplyEmailToConfig();
                var validationError = ValidateEmailFields();
                if (validationError != null)
                {
                    _lblEmailResult.ForeColor = Color.FromArgb(255, 100, 100);
                    _lblEmailResult.Text = validationError;
                    return;
                }
                await Task.Run(() => EmailNotifier.SendTestEmail(_cfg));
                _lblEmailResult.ForeColor = ColorTranslator.FromHtml("#1F6F3A");
                _lblEmailResult.Text = "✓ Test email sent successfully.";
            }
            catch (Exception ex)
            {
                _lblEmailResult.ForeColor = Color.FromArgb(255, 100, 100);
                _lblEmailResult.Text = $"✖ {ex.Message}";
            }
        };

        _btnValidateSmtp.Click += async (_, _) =>
        {
            _lblEmailResult.ForeColor = Color.Gray;
            _lblEmailResult.Text = "Checking SMTP connectivity…";
            _lblEmailResult.Visible = true;
            try
            {
                var (ok, msg) = await Task.Run(() =>
                    EmailNotifier.ValidateSmtpConnectivity(_txtSmtpServer.Text.Trim(), (int)_nudSmtpPort.Value));
                _lblEmailResult.ForeColor = ok ? ColorTranslator.FromHtml("#1F6F3A") : Color.FromArgb(255, 100, 100);
                _lblEmailResult.Text = ok ? $"✓ {msg}" : $"✖ {msg}";
            }
            catch (Exception ex)
            {
                _lblEmailResult.ForeColor = Color.FromArgb(255, 100, 100);
                _lblEmailResult.Text = $"✖ {ex.Message}";
            }
        };
    }

    private void BuildEmailPanel()
    {
        var tbl = MakeTable(4, 8);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));

        int r = 0;
        tbl.Controls.Add(MakeLabel("SMTP Server:"), 0, r);
        _txtSmtpServer.Dock = DockStyle.Fill; tbl.Controls.Add(_txtSmtpServer, 1, r);
        tbl.Controls.Add(MakeLabel("Port:"), 2, r);
        _nudSmtpPort.Dock = DockStyle.Fill; tbl.Controls.Add(_nudSmtpPort, 3, r);

        r++;
        tbl.SetColumnSpan(_chkSsl, 4);
        tbl.Controls.Add(_chkSsl, 0, r);

        r++;
        tbl.Controls.Add(MakeLabel("From:"), 0, r);
        _txtFrom.Dock = DockStyle.Fill; tbl.SetColumnSpan(_txtFrom, 3); tbl.Controls.Add(_txtFrom, 1, r);

        r++;
        tbl.Controls.Add(MakeLabel("To (;-sep):"), 0, r);
        _txtTo.Dock = DockStyle.Fill; tbl.SetColumnSpan(_txtTo, 3); tbl.Controls.Add(_txtTo, 1, r);

        r++;
        tbl.Controls.Add(MakeLabel("CC (;-sep):"), 0, r);
        _txtCc.Dock = DockStyle.Fill; tbl.SetColumnSpan(_txtCc, 3); tbl.Controls.Add(_txtCc, 1, r);

        r++;
        tbl.Controls.Add(MakeLabel("SMTP User:"), 0, r);
        _txtSmtpUser.Dock = DockStyle.Fill; tbl.Controls.Add(_txtSmtpUser, 1, r);
        tbl.Controls.Add(MakeLabel("Password:"), 2, r);
        _txtSmtpPass.Dock = DockStyle.Fill; tbl.Controls.Add(_txtSmtpPass, 3, r);

        r++;
        var flow = MakeButtonFlow(_btnTestEmail, _btnValidateSmtp);
        tbl.SetColumnSpan(flow, 4);
        tbl.Controls.Add(flow, 0, r);

        r++;
        tbl.SetColumnSpan(_lblEmailResult, 4);
        tbl.Controls.Add(_lblEmailResult, 0, r);

        _pnlEmail.Controls.Add(tbl);
        _pnlEmail.Visible = false;
    }

    private void BuildWebhookPanel()
    {
        var tbl = MakeTable(2, 1);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        tbl.Controls.Add(MakeLabel("Webhook URL:"), 0, 0);
        _txtWebhookUrl.Dock = DockStyle.Fill;
        tbl.Controls.Add(_txtWebhookUrl, 1, 0);
        _pnlWebhook.Controls.Add(tbl);
        _pnlWebhook.Visible = false;
    }

    private void BuildMaintenancePanel()
    {
        var tbl = MakeTable(4, 4);
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        tbl.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));

        int r = 0;
        tbl.Controls.Add(MakeLabel("Max Log Size (MB):"), 0, r);
        _nudMaxLogSizeMB.Dock = DockStyle.Left; tbl.Controls.Add(_nudMaxLogSizeMB, 1, r);
        tbl.Controls.Add(MakeLabel("Max Log Files:"), 2, r);
        _nudMaxLogFiles.Dock = DockStyle.Left; tbl.Controls.Add(_nudMaxLogFiles, 3, r);

        r++;
        tbl.Controls.Add(MakeLabel("Dump Retention (days):"), 0, r);
        _nudRetentionDays.Dock = DockStyle.Left; tbl.Controls.Add(_nudRetentionDays, 1, r);
        tbl.Controls.Add(MakeLabel("Max Dump GB:"), 2, r);
        _nudRetentionMaxGB.Dock = DockStyle.Left; tbl.Controls.Add(_nudRetentionMaxGB, 3, r);

        r++;
        tbl.Controls.Add(MakeLabel("Stability Timeout (s):"), 0, r);
        _nudStabilityTimeout.Dock = DockStyle.Left; tbl.Controls.Add(_nudStabilityTimeout, 1, r);

        r++;
        var hintLabel = new Label
        {
            Text = "Retention: 0 = disabled. Stability timeout: how long to wait for exclusive lock on dump file.",
            AutoSize = true,
            ForeColor = Color.Gray,
            Margin = new Padding(0, 2, 0, 0)
        };
        tbl.SetColumnSpan(hintLabel, 4);
        tbl.Controls.Add(hintLabel, 0, r);

        _pnlMaintenance.Controls.Add(tbl);
    }

    public override void OnEnter(Config cfg)
    {
        _cfg = cfg;

        _chkEmailEnabled.Checked = cfg.EmailEnabled;
        _txtSmtpServer.Text = cfg.SmtpServer;
        _nudSmtpPort.Value = Math.Clamp(cfg.SmtpPort, 1, 65535);
        _chkSsl.Checked = cfg.UseSsl;
        _txtFrom.Text = cfg.FromAddress;
        _txtTo.Text = cfg.ToAddress;
        _txtCc.Text = cfg.CcAddress;
        _txtSmtpUser.Text = cfg.SmtpUsername;
        if (!string.IsNullOrEmpty(cfg.EncryptedPasswordBlob))
            _txtSmtpPass.PlaceholderText = "(stored securely)";

        _chkWebhookEnabled.Checked = cfg.WebhookEnabled;
        _txtWebhookUrl.Text = cfg.WebhookUrl;

        _nudMaxLogSizeMB.Value = Math.Clamp(cfg.MaxLogSizeMB, 1, 1000);
        _nudMaxLogFiles.Value = Math.Clamp(cfg.MaxLogFiles, 1, 100);
        _nudRetentionDays.Value = Math.Clamp(cfg.DumpRetentionDays, 0, 3650);
        _nudRetentionMaxGB.Value = Math.Clamp((decimal)cfg.DumpRetentionMaxGB, 0, 10000);
        _nudStabilityTimeout.Value = Math.Clamp(cfg.DumpStabilityTimeoutSeconds, 5, 300);

        _pnlEmail.Visible = _chkEmailEnabled.Checked;
        _pnlWebhook.Visible = _chkWebhookEnabled.Checked;
        _lblEmailResult.Visible = false;
        _lblEmailValidation.Visible = false;
    }

    public override bool OnLeave(Config cfg)
    {
        ApplyEmailToConfig();

        // Validate email if enabled
        if (_chkEmailEnabled.Checked)
        {
            var error = ValidateEmailFields();
            if (error != null)
            {
                _lblEmailValidation.Text = error;
                _lblEmailValidation.Visible = true;
                return false;
            }
        }

        // Write all values to cfg
        cfg.EmailEnabled = _chkEmailEnabled.Checked;
        cfg.SmtpServer = _txtSmtpServer.Text.Trim();
        cfg.SmtpPort = (int)_nudSmtpPort.Value;
        cfg.UseSsl = _chkSsl.Checked;
        cfg.FromAddress = _txtFrom.Text.Trim();
        cfg.ToAddress = _txtTo.Text.Trim();
        cfg.CcAddress = _txtCc.Text.Trim();
        cfg.SmtpUsername = _txtSmtpUser.Text.Trim();

        string passText = _txtSmtpPass.Text;
        if (!string.IsNullOrEmpty(passText))
            cfg.SetPassword(passText);

        cfg.WebhookEnabled = _chkWebhookEnabled.Checked;
        cfg.WebhookUrl = _txtWebhookUrl.Text.Trim();

        cfg.MaxLogSizeMB = (int)_nudMaxLogSizeMB.Value;
        cfg.MaxLogFiles = (int)_nudMaxLogFiles.Value;
        cfg.DumpRetentionDays = (int)_nudRetentionDays.Value;
        cfg.DumpRetentionMaxGB = (double)_nudRetentionMaxGB.Value;
        cfg.DumpStabilityTimeoutSeconds = (int)_nudStabilityTimeout.Value;

        return true;
    }

    public override bool IsValid() => true; // Notifications are optional; validated on leave if enabled

    private void ApplyEmailToConfig()
    {
        _cfg.EmailEnabled = _chkEmailEnabled.Checked;
        _cfg.SmtpServer = _txtSmtpServer.Text.Trim();
        _cfg.SmtpPort = (int)_nudSmtpPort.Value;
        _cfg.UseSsl = _chkSsl.Checked;
        _cfg.FromAddress = _txtFrom.Text.Trim();
        _cfg.ToAddress = _txtTo.Text.Trim();
        _cfg.CcAddress = _txtCc.Text.Trim();
        _cfg.SmtpUsername = _txtSmtpUser.Text.Trim();

        string passText = _txtSmtpPass.Text;
        if (!string.IsNullOrEmpty(passText))
            _cfg.SetPassword(passText);
    }

    private string? ValidateEmailFields()
    {
        string from = _txtFrom.Text.Trim();
        if (string.IsNullOrWhiteSpace(from) || !from.Contains('@'))
            return "From address is not a valid email.";

        var (toOk, toErr) = EmailNotifier.ValidateAddressList(_txtTo.Text.Trim(), "To");
        if (!toOk) return toErr;

        string cc = _txtCc.Text.Trim();
        if (!string.IsNullOrWhiteSpace(cc))
        {
            var (ccOk, ccErr) = EmailNotifier.ValidateAddressList(cc, "CC");
            if (!ccOk) return ccErr;
        }

        if (string.IsNullOrWhiteSpace(_txtSmtpServer.Text))
            return "SMTP server is required.";

        return null;
    }
}

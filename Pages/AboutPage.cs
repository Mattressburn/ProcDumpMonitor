namespace ProcDumpMonitor;

/// <summary>Wizard Step 6 — About information.</summary>
public sealed class AboutPage : WizardPage
{
    public override string StepTitle => "About";

    public AboutPage()
    {
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 4,
            Margin = Padding.Empty
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        // Row 0 — Logo (PNG from embedded resource, or emoji placeholder)
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 210));
        // Assets\logo.svg exists but WinForms cannot render SVG natively;
        // a PNG fallback (Assets\logo.png) is needed for the embedded resource.
        var logoStream = typeof(AboutPage).Assembly
            .GetManifestResourceStream("ProcDumpMonitor.logo.png");

        Control logoControl;
        if (logoStream != null)
        {
            var pic = new PictureBox
            {
                Size = new Size(200, 200),
                SizeMode = PictureBoxSizeMode.Zoom,
                Image = System.Drawing.Image.FromStream(logoStream),
                Anchor = AnchorStyles.None
            };
            logoControl = pic;
        }
        else
        {
            logoControl = new Label
            {
                Text = "\U0001F310",
                Font = new Font("Segoe UI", 48f),
                AutoSize = true,
                TextAlign = ContentAlignment.MiddleCenter,
                Anchor = AnchorStyles.None
            };
        }
        layout.Controls.Add(logoControl, 0, 0);

        // Row 1 — App name
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var lblName = new Label
        {
            Text = "ProcDumpMonitor",
            Font = new Font("Segoe UI", 16f, FontStyle.Bold),
            AutoSize = true,
            TextAlign = ContentAlignment.MiddleCenter,
            Anchor = AnchorStyles.None
        };
        layout.Controls.Add(lblName, 0, 1);

        // Row 2 — Author
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var lblAuthor = new Label
        {
            Text = "Matthew Raburn",
            Font = new Font("Segoe UI", 11f),
            AutoSize = true,
            TextAlign = ContentAlignment.MiddleCenter,
            Anchor = AnchorStyles.None
        };
        layout.Controls.Add(lblAuthor, 0, 2);

        // Row 3 — Version / build date
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var lblVersion = new Label
        {
            Text = $"Version {BuildInfo.BuildDate}",
            Font = new Font("Segoe UI", 9.5f, FontStyle.Italic),
            AutoSize = true,
            TextAlign = ContentAlignment.MiddleCenter,
            Anchor = AnchorStyles.None
        };
        layout.Controls.Add(lblVersion, 0, 3);

        Controls.Add(layout);
    }

    public override bool IsValid() => true;
    public override bool OnLeave(Config cfg) => true;
}

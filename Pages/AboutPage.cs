namespace ProcDumpMonitor;

/// <summary>Wizard Step 6 -- About information.</summary>
public sealed class AboutPage : WizardPage
{
    public override string StepTitle => "About";

    public AboutPage()
    {
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 5,
            Margin = Padding.Empty
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        // Row 0 -- Logo (single centered logo from embedded resource)
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 180));
        layout.Controls.Add(BuildLogoControl(), 0, 0);

        // Row 1 -- App name
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var lblName = new Label
        {
            Text = "ProcDump Monitor",
            Font = new Font("Segoe UI", 16f, FontStyle.Bold),
            AutoSize = true,
            TextAlign = ContentAlignment.MiddleCenter,
            Anchor = AnchorStyles.None
        };
        layout.Controls.Add(lblName, 0, 1);

        // Row 2 -- Attribution
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var lblAuthor = new Label
        {
            Text = "A SWH L3 Production",
            Font = new Font("Segoe UI", 11f),
            AutoSize = true,
            TextAlign = ContentAlignment.MiddleCenter,
            Anchor = AnchorStyles.None
        };
        layout.Controls.Add(lblAuthor, 0, 2);

        // Row 3 -- Version / build date
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

        // Row 4 -- spacer absorbs remaining space
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.Controls.Add(new Panel(), 0, 4);

        Controls.Add(layout);
    }

    /// <summary>Builds a centered PictureBox for the JCI globe logo, or a fallback label.</summary>
    private static Control BuildLogoControl()
    {
        try
        {
            var stream = typeof(AboutPage).Assembly
                .GetManifestResourceStream("ProcDumpMonitor.jci_globe_256.png");
            if (stream is not null)
            {
                return new PictureBox
                {
                    Size = new Size(160, 160),
                    SizeMode = PictureBoxSizeMode.Zoom,
                    Image = Image.FromStream(stream),
                    Anchor = AnchorStyles.None
                };
            }
        }
        catch
        {
            // Fall through to fallback label
        }

        return new Label
        {
            Text = "Logo unavailable",
            Font = new Font("Segoe UI", 10f, FontStyle.Italic),
            AutoSize = true,
            TextAlign = ContentAlignment.MiddleCenter,
            Anchor = AnchorStyles.None
        };
    }

    public override bool IsValid() => true;
    public override bool OnLeave(Config cfg) => true;
}

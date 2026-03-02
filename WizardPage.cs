namespace ProcDumpMonitor;

/// <summary>Base class for all wizard pages.</summary>
public abstract class WizardPage : UserControl
{
    protected WizardPage()
    {
        Dock = DockStyle.Fill;
        AutoScroll = true;
        Padding = new Padding(24, 16, 24, 16);
        AutoScaleMode = AutoScaleMode.Dpi;
    }

    /// <summary>Display name shown in the step indicator.</summary>
    public abstract string StepTitle { get; }

    /// <summary>Called when the page becomes active. Load/refresh data here.</summary>
    public virtual void OnEnter(Config cfg) { }

    /// <summary>Called when leaving the page. Write values to cfg. Return false to block navigation.</summary>
    public virtual bool OnLeave(Config cfg) => true;

    /// <summary>Validate current inputs. Return true if Next should be enabled.</summary>
    public abstract bool IsValid();

    /// <summary>Raised when validation state changes so the wizard can update Next button.</summary>
    public event EventHandler? ValidationChanged;

    protected void RaiseValidationChanged() => ValidationChanged?.Invoke(this, EventArgs.Empty);

    // ── Shared layout helpers (same style as old MainForm) ──

    protected static Label MakeLabel(string text) => new()
    {
        Text = text,
        AutoSize = true,
        Anchor = AnchorStyles.Left,
        Margin = new Padding(0, 6, 6, 0)
    };

    protected static Label MakeValidationLabel() => new()
    {
        AutoSize = true,
        ForeColor = Color.FromArgb(255, 100, 100),
        Visible = false,
        Margin = new Padding(0, 4, 0, 0)
    };

    protected static GroupBox MakeGroup(string text) => new()
    {
        Text = text,
        Dock = DockStyle.Fill,
        AutoSize = true,
        AutoSizeMode = AutoSizeMode.GrowAndShrink,
        Padding = new Padding(10, 6, 10, 8),
        Margin = new Padding(0, 0, 0, 4)
    };

    protected static TableLayoutPanel MakeTable(int cols, int rows)
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

    protected static FlowLayoutPanel MakeButtonFlow(params Button[] buttons)
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
}

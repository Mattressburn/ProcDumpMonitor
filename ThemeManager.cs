namespace ProcDumpMonitor;

/// <summary>
/// Applies a VS Code–style dark theme to any WinForms control tree.
/// </summary>
public static class ThemeManager
{
    // ── Palette ──
    public static readonly Color Background     = ColorTranslator.FromHtml("#1E1E1E");
    public static readonly Color PanelBackground = ColorTranslator.FromHtml("#252526");
    public static readonly Color Foreground      = ColorTranslator.FromHtml("#E6E6E6");
    public static readonly Color Border          = ColorTranslator.FromHtml("#3C3C3C");
    public static readonly Color Accent          = ColorTranslator.FromHtml("#0E639C");
    public static readonly Color InputBackground = ColorTranslator.FromHtml("#2D2D30");
    public static readonly Color DisabledText    = ColorTranslator.FromHtml("#808080");
    public static readonly Color StatusBackground = ColorTranslator.FromHtml("#1B1B1B");

    /// <summary>Apply the dark theme to <paramref name="root"/> and all descendants.</summary>
    public static void ApplyTheme(Control root)
    {
        if (root is Form form)
        {
            form.BackColor = Background;
            form.ForeColor = Foreground;
        }

        ApplyRecursive(root);
    }

    private static void ApplyRecursive(Control control)
    {
        switch (control)
        {
            case Button btn:
                // Skip buttons inside the banner panel (they use dynamic colours)
                if (btn.Parent is Panel bp && bp.Name == "BannerPanel")
                    break;
                btn.FlatStyle = FlatStyle.Flat;
                btn.BackColor = Accent;
                btn.ForeColor = Color.White;
                btn.FlatAppearance.BorderColor = Border;
                btn.FlatAppearance.BorderSize = 1;
                btn.FlatAppearance.MouseOverBackColor = ControlPaint.Light(Accent, 0.15f);
                btn.FlatAppearance.MouseDownBackColor = ControlPaint.Dark(Accent, 0.15f);
                btn.Cursor = Cursors.Hand;
                break;

            case TextBox txt:
                txt.BackColor = InputBackground;
                txt.ForeColor = Foreground;
                txt.BorderStyle = BorderStyle.FixedSingle;
                break;

            case RichTextBox rtb:
                rtb.BackColor = StatusBackground;
                rtb.ForeColor = Foreground;
                rtb.BorderStyle = BorderStyle.None;
                break;

            case ComboBox cbo:
                cbo.BackColor = InputBackground;
                cbo.ForeColor = Foreground;
                cbo.FlatStyle = FlatStyle.Flat;
                break;

            case NumericUpDown nud:
                nud.BackColor = InputBackground;
                nud.ForeColor = Foreground;
                break;

            case ThemedCheckBox:
                // ThemedCheckBox owner-draws itself — skip theme overrides
                break;

            case CheckBox chk:
                chk.ForeColor = Foreground;
                chk.FlatStyle = FlatStyle.Flat;
                chk.FlatAppearance.CheckedBackColor = Accent;
                break;

            case GroupBox grp:
                grp.ForeColor = Foreground;
                grp.BackColor = PanelBackground;
                break;

            case Label lbl:
                // Preserve the elevation warning colour and banner label colours
                if (lbl.ForeColor == Color.OrangeRed || lbl.ForeColor == Color.FromArgb(255, 69, 0))
                    break;
                if (lbl.Parent is Panel p && p.Name == "BannerPanel")
                    break;
                lbl.ForeColor = Foreground;
                break;

            case TableLayoutPanel tlp:
                tlp.BackColor = Color.Transparent;
                break;

            case FlowLayoutPanel flp:
                flp.BackColor = Color.Transparent;
                break;

            case Panel pnl:
                // Don't override the banner panel — it uses dynamic success/error colours
                if (pnl.Name == "BannerPanel")
                    break;
                // ScrollHost needs the background colour, not transparent
                if (pnl.Name == "ScrollHost")
                {
                    pnl.BackColor = Background;
                    break;
                }
                pnl.BackColor = Color.Transparent;
                break;

            case SplitContainer sc:
                sc.BackColor = Background;
                sc.Panel1.BackColor = Background;
                sc.Panel2.BackColor = Background;
                break;
        }

        foreach (Control child in control.Controls)
        {
            ApplyRecursive(child);
        }
    }
}

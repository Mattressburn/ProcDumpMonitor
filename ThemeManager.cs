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

    /// <summary>Apply the dark theme to a <see cref="ToolStrip"/> and its items.</summary>
    public static void ApplyTheme(ToolStrip strip)
    {
        strip.BackColor = PanelBackground;
        strip.ForeColor = Foreground;
        strip.Renderer = new ToolStripProfessionalRenderer(new DarkColorTable())
        {
            RoundedEdges = false
        };
        ApplyToolStripItems(strip.Items);
    }

    /// <summary>Recursively theme all items in a ToolStrip, including nested dropdown menus.</summary>
    private static void ApplyToolStripItems(ToolStripItemCollection items)
    {
        foreach (ToolStripItem item in items)
        {
            item.BackColor = PanelBackground;
            item.ForeColor = Foreground;

            if (item is ToolStripMenuItem menuItem && menuItem.HasDropDown)
            {
                menuItem.DropDown.BackColor = PanelBackground;
                menuItem.DropDown.ForeColor = Foreground;
                ApplyToolStripItems(menuItem.DropDownItems);
            }
        }
    }

    /// <summary>
    /// Enable the Windows 10/11 dark title bar (build 18985+).
    /// Falls back silently on older Windows versions.
    /// </summary>
    public static void EnableDarkTitleBar(Form form)
    {
        try
        {
            // DWMWA_USE_IMMERSIVE_DARK_MODE = 20 (Windows 10 20H1+)
            int attribute = 20;
            int value = 1;
            DwmSetWindowAttribute(form.Handle, attribute, ref value, sizeof(int));
        }
        catch
        {
            // Silently fail on older Windows versions
        }
    }

    [System.Runtime.InteropServices.DllImport("dwmapi.dll", PreserveSig = true)]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int value, int size);

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
                // Don't override the status banner — it uses dynamic success/error colours
                if (pnl.Name == "StatusBanner")
                    break;
                // ScrollHost / WizardContent need the background colour, not transparent
                if (pnl.Name == "ScrollHost" || pnl.Name == "WizardContent")
                {
                    pnl.BackColor = Background;
                    break;
                }
                pnl.BackColor = Color.Transparent;
                break;

            case StepIndicator si:
                si.BackColor = Background;
                break;

            case SplitContainer sc:
                sc.BackColor = Background;
                sc.Panel1.BackColor = Background;
                sc.Panel2.BackColor = Background;
                break;

            case ToolStrip ts:
                // Theme any MenuStrip or ToolStrip found in the control tree
                ApplyTheme(ts);
                break;
        }

        foreach (Control child in control.Controls)
        {
            ApplyRecursive(child);
        }
    }

    private sealed class DarkColorTable : ProfessionalColorTable
    {
        // Top-level menu strip background
        public override Color MenuStripGradientBegin => PanelBackground;
        public override Color MenuStripGradientEnd => PanelBackground;

        // Hover / selection highlight on menu items
        public override Color MenuItemSelected => Accent;
        public override Color MenuItemBorder => Border;
        public override Color MenuItemSelectedGradientBegin => Accent;
        public override Color MenuItemSelectedGradientEnd => Accent;
        public override Color MenuItemPressedGradientBegin => PanelBackground;
        public override Color MenuItemPressedGradientEnd => PanelBackground;

        // Dropdown background and image margin (gutter)
        public override Color ToolStripDropDownBackground => PanelBackground;
        public override Color ImageMarginGradientBegin => PanelBackground;
        public override Color ImageMarginGradientMiddle => PanelBackground;
        public override Color ImageMarginGradientEnd => PanelBackground;

        // Dropdown border
        public override Color MenuBorder => Border;

        // Separators
        public override Color SeparatorDark => Border;
        public override Color SeparatorLight => Border;

        // Check mark background (checked menu items)
        public override Color CheckBackground => Accent;
        public override Color CheckSelectedBackground => ControlPaint.Light(Accent, 0.15f);
        public override Color CheckPressedBackground => ControlPaint.Dark(Accent, 0.15f);

        // ToolStrip content panel (hosted controls area)
        public override Color ToolStripContentPanelGradientBegin => Background;
        public override Color ToolStripContentPanelGradientEnd => Background;

        // ToolStrip panel / status strip gradients
        public override Color ToolStripPanelGradientBegin => PanelBackground;
        public override Color ToolStripPanelGradientEnd => PanelBackground;
        public override Color StatusStripGradientBegin => PanelBackground;
        public override Color StatusStripGradientEnd => PanelBackground;

        // Standard toolbar gradient
        public override Color ToolStripGradientBegin => PanelBackground;
        public override Color ToolStripGradientMiddle => PanelBackground;
        public override Color ToolStripGradientEnd => PanelBackground;

        // Overflow button
        public override Color OverflowButtonGradientBegin => PanelBackground;
        public override Color OverflowButtonGradientMiddle => PanelBackground;
        public override Color OverflowButtonGradientEnd => PanelBackground;

        // Grip (drag handle)
        public override Color GripDark => Border;
        public override Color GripLight => PanelBackground;

        // Button states
        public override Color ButtonSelectedHighlight => Accent;
        public override Color ButtonSelectedHighlightBorder => Border;
        public override Color ButtonPressedHighlight => ControlPaint.Dark(Accent, 0.15f);
        public override Color ButtonPressedHighlightBorder => Border;
        public override Color ButtonCheckedHighlight => Accent;
        public override Color ButtonCheckedHighlightBorder => Border;
        public override Color ButtonSelectedBorder => Border;
        public override Color ButtonSelectedGradientBegin => Accent;
        public override Color ButtonSelectedGradientEnd => Accent;
        public override Color ButtonPressedGradientBegin => ControlPaint.Dark(Accent, 0.15f);
        public override Color ButtonPressedGradientEnd => ControlPaint.Dark(Accent, 0.15f);
        public override Color ButtonCheckedGradientBegin => Accent;
        public override Color ButtonCheckedGradientEnd => Accent;

        // Rafting container
        public override Color RaftingContainerGradientBegin => Background;
        public override Color RaftingContainerGradientEnd => Background;
    }
}

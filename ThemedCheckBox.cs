using System.Drawing;
using System.Drawing.Drawing2D;

namespace ProcDumpMonitor;

/// <summary>
/// Owner-drawn CheckBox with high-contrast dark-theme visuals.
/// Renders a custom glyph so the checked state is always clearly visible.
/// </summary>
public class ThemedCheckBox : CheckBox
{
    // ── Palette (matches ThemeManager) ──
    private static readonly Color BoxFill          = ColorTranslator.FromHtml("#1E1E1E");
    private static readonly Color BorderUnchecked  = ColorTranslator.FromHtml("#6A6A6A");
    private static readonly Color BorderChecked    = ColorTranslator.FromHtml("#0E639C");
    private static readonly Color CheckMarkColor   = Color.White;
    private static readonly Color TextColor        = ColorTranslator.FromHtml("#E6E6E6");
    private static readonly Color FocusBorderColor = ColorTranslator.FromHtml("#0E639C");

    private const int GlyphSize = 16;
    private const int GlyphTextGap = 6;

    public ThemedCheckBox()
    {
        SetStyle(
            ControlStyles.UserPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.AllPaintingInWmPaint |
            ControlStyles.ResizeRedraw,
            true);

        AutoSize = true;
        MinimumSize = new Size(0, GlyphSize + 4);
    }

    protected override void OnCheckedChanged(EventArgs e)
    {
        base.OnCheckedChanged(e);
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        var g = e.Graphics;
        g.SmoothingMode = SmoothingMode.HighQuality;
        g.PixelOffsetMode = PixelOffsetMode.HighQuality;

        // Clear with parent background
        Color bg = Parent?.BackColor ?? BoxFill;
        g.Clear(bg);

        // ── Glyph box ──
        int glyphY = (Height - GlyphSize) / 2;
        var boxRect = new Rectangle(0, glyphY, GlyphSize, GlyphSize);

        // Fill
        using (var fillBrush = new SolidBrush(BoxFill))
            g.FillRectangle(fillBrush, boxRect);

        // Border
        Color borderColor = Checked ? BorderChecked : BorderUnchecked;
        float borderWidth = Checked ? 2f : 1.5f;
        using (var borderPen = new Pen(borderColor, borderWidth))
        {
            // Inset slightly so the pen doesn't clip
            var borderRect = new RectangleF(
                boxRect.X + borderWidth / 2f,
                boxRect.Y + borderWidth / 2f,
                boxRect.Width - borderWidth,
                boxRect.Height - borderWidth);
            g.DrawRectangle(borderPen, borderRect.X, borderRect.Y, borderRect.Width, borderRect.Height);
        }

        // ── Checkmark ──
        if (Checked)
        {
            // Fill the box with accent when checked for extra contrast
            var innerRect = new RectangleF(
                boxRect.X + 2f, boxRect.Y + 2f,
                boxRect.Width - 4f, boxRect.Height - 4f);
            using (var accentBrush = new SolidBrush(BorderChecked))
                g.FillRectangle(accentBrush, innerRect);

            // Draw a ✓ shape
            float cx = boxRect.X + GlyphSize / 2f;
            float cy = boxRect.Y + GlyphSize / 2f;
            float s = GlyphSize * 0.28f;  // scale factor

            var checkPoints = new PointF[]
            {
                new(cx - s * 1.0f, cy - s * 0.1f),
                new(cx - s * 0.2f, cy + s * 0.8f),
                new(cx + s * 1.1f, cy - s * 0.9f)
            };

            using var checkPen = new Pen(CheckMarkColor, 2.2f)
            {
                StartCap = LineCap.Round,
                EndCap = LineCap.Round,
                LineJoin = LineJoin.Round
            };
            g.DrawLines(checkPen, checkPoints);
        }

        // ── Text ──
        int textX = GlyphSize + GlyphTextGap;
        Color foreColor = Enabled ? TextColor : ColorTranslator.FromHtml("#808080");
        TextRenderer.DrawText(g, Text, Font, new Point(textX, (Height - Font.Height) / 2),
            foreColor, bg, TextFormatFlags.Left | TextFormatFlags.SingleLine);

        // ── Focus rectangle ──
        if (Focused && ShowFocusCues)
        {
            var textSize = TextRenderer.MeasureText(g, Text, Font);
            var focusRect = new Rectangle(textX - 1, (Height - textSize.Height) / 2 - 1,
                textSize.Width + 2, textSize.Height + 2);
            using var focusPen = new Pen(FocusBorderColor) { DashStyle = DashStyle.Dot };
            g.DrawRectangle(focusPen, focusRect);
        }
    }

    public override Size GetPreferredSize(Size proposedSize)
    {
        if (string.IsNullOrEmpty(Text))
            return new Size(GlyphSize, GlyphSize + 4);

        using var g = CreateGraphics();
        var textSize = TextRenderer.MeasureText(g, Text, Font);
        return new Size(GlyphSize + GlyphTextGap + textSize.Width + 2,
            Math.Max(GlyphSize + 4, textSize.Height + 4));
    }
}

using System.Drawing.Drawing2D;

namespace ProcDumpMonitor;

/// <summary>
/// Owner-drawn step indicator bar. Renders numbered circles
/// connected by lines with completed / current / future states.
/// </summary>
public sealed class StepIndicator : Control
{
    private string[] _stepTitles = [];
    private int _currentStep;
    private readonly HashSet<int> _completedSteps = [];
    private readonly HashSet<int> _errorSteps = [];

    private static readonly Color CompletedFill = ColorTranslator.FromHtml("#1F6F3A");
    private static readonly Color CurrentRing = ThemeManager.Accent;
    private static readonly Color FutureFill = ColorTranslator.FromHtml("#3C3C3C");
    private static readonly Color ErrorFill = ColorTranslator.FromHtml("#8B1E1E");
    private static readonly Color TextColor = ThemeManager.Foreground;
    private static readonly Color DimTextColor = ThemeManager.DisabledText;
    private static readonly Color LineColor = ThemeManager.Border;

    private const int CircleDiameter = 28;
    private const int LineHeight = 2;

    public StepIndicator()
    {
        SetStyle(
            ControlStyles.UserPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.AllPaintingInWmPaint |
            ControlStyles.ResizeRedraw,
            true);
        // Height and Dock are set by the parent layout container.
        // MinimumSize ensures the indicator never collapses to zero.
        MinimumSize = new Size(0, 52);
    }

    public void Configure(string[] stepTitles)
    {
        _stepTitles = stepTitles;
        Invalidate();
    }

    public void SetCurrentStep(int index)
    {
        _currentStep = index;
        Invalidate();
    }

    public void MarkCompleted(int index)
    {
        _completedSteps.Add(index);
        _errorSteps.Remove(index);
        Invalidate();
    }

    public void MarkError(int index)
    {
        _errorSteps.Add(index);
        _completedSteps.Remove(index);
        Invalidate();
    }

    public void ClearMark(int index)
    {
        _completedSteps.Remove(index);
        _errorSteps.Remove(index);
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        var g = e.Graphics;
        g.SmoothingMode = SmoothingMode.HighQuality;
        g.Clear(Parent?.BackColor ?? ThemeManager.Background);

        if (_stepTitles.Length == 0) return;

        int count = _stepTitles.Length;
        int padX = Padding.Left + Padding.Right + CircleDiameter;
        float totalWidth = Width - padX;
        float spacing = count > 1 ? totalWidth / (count - 1) : 0;
        float startX = Padding.Left + CircleDiameter / 2f;
        float centerY = Height / 2f - 2;

        // Draw connecting lines
        using var linePen = new Pen(LineColor, LineHeight);
        for (int i = 0; i < count - 1; i++)
        {
            float x1 = startX + i * spacing + CircleDiameter / 2f + 2;
            float x2 = startX + (i + 1) * spacing - CircleDiameter / 2f - 2;
            if (x2 > x1)
                g.DrawLine(linePen, x1, centerY, x2, centerY);
        }

        // Draw circles and labels
        using var completedBrush = new SolidBrush(CompletedFill);
        using var errorBrush = new SolidBrush(ErrorFill);
        using var futureBrush = new SolidBrush(FutureFill);
        using var currentPen = new Pen(CurrentRing, 2.5f);
        using var textBrush = new SolidBrush(TextColor);
        using var dimTextBrush = new SolidBrush(DimTextColor);
        using var whiteBrush = new SolidBrush(Color.White);
        using var numFont = new Font(Font.FontFamily, 9f, FontStyle.Bold);
        using var labelFont = new Font(Font.FontFamily, 7.5f, FontStyle.Regular);

        for (int i = 0; i < count; i++)
        {
            float cx = startX + i * spacing;
            var circleRect = new RectangleF(
                cx - CircleDiameter / 2f,
                centerY - CircleDiameter / 2f - 4,
                CircleDiameter,
                CircleDiameter);

            if (_completedSteps.Contains(i))
                g.FillEllipse(completedBrush, circleRect);
            else if (_errorSteps.Contains(i))
                g.FillEllipse(errorBrush, circleRect);
            else if (i == _currentStep)
            {
                g.FillEllipse(futureBrush, circleRect);
                g.DrawEllipse(currentPen, circleRect);
            }
            else
                g.FillEllipse(futureBrush, circleRect);

            // Number or checkmark
            string circleText = _completedSteps.Contains(i) ? "✓" :
                                _errorSteps.Contains(i) ? "!" :
                                (i + 1).ToString();
            var textSize = g.MeasureString(circleText, numFont);
            g.DrawString(circleText, numFont, whiteBrush,
                circleRect.X + (circleRect.Width - textSize.Width) / 2f,
                circleRect.Y + (circleRect.Height - textSize.Height) / 2f);

            // Label below circle
            var labelSize = g.MeasureString(_stepTitles[i], labelFont);
            var labelBrush2 = i == _currentStep ? textBrush : dimTextBrush;
            g.DrawString(_stepTitles[i], labelFont, labelBrush2,
                cx - labelSize.Width / 2f,
                circleRect.Bottom + 2);
        }
    }
}

using System.Text.RegularExpressions;

namespace ProcDumpMonitor;

/// <summary>
/// Sanitises task names for the Windows Task Scheduler.
/// Invalid characters: \ / : * ? " &lt; &gt; |
/// </summary>
internal static partial class TaskNameHelper
{
    // Characters the Task Scheduler does not allow in task names.
    private static readonly char[] InvalidChars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

    /// <summary>
    /// Produce a clean task name:
    ///   1. Replace every invalid character with a dash.
    ///   2. Collapse runs of whitespace / dashes into a single space.
    ///   3. Trim leading/trailing whitespace.
    ///   4. Cap at 200 characters (well under the ~260-char path limit).
    /// </summary>
    public static string Sanitize(string raw)
    {
        if (string.IsNullOrWhiteSpace(raw))
            return raw?.Trim() ?? string.Empty;

        // Step 1 – replace invalid chars
        var chars = raw.ToCharArray();
        for (int i = 0; i < chars.Length; i++)
        {
            if (Array.IndexOf(InvalidChars, chars[i]) >= 0)
                chars[i] = '-';
        }

        // Step 2 – collapse repeated whitespace / dashes
        string result = CollapseWhitespaceDash().Replace(new string(chars), " ");

        // Step 3 – trim
        result = result.Trim();

        // Step 4 – cap length
        if (result.Length > 200)
            result = result[..200].TrimEnd();

        return result;
    }

    [GeneratedRegex(@"[\s\-]{2,}")]
    private static partial Regex CollapseWhitespaceDash();
}

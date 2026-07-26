# GUI end-to-end driver for ProcDump Monitor (Rust build, mode-based shell).
# Launches the exe (build it with PDM_TEST_MANIFEST=1 so no UAC prompt), drives
# it like a real user - physical mouse clicks + keystrokes - screenshotting
# every page. Navigation is now freely-clickable SIDEBAR labels (not Back/Next).
# nwg controls expose no UIA patterns, so UIA is used only to FIND elements
# (class+name+rect); input is synthesized and ambiguous pixels are adjudicated
# with deterministic window-message probes (CB_GETCOUNT). Also runs a real
# System Health collection and captures the app log + run transcript.
# Run under Windows PowerShell 5.1 (powershell.exe) for UIA assemblies. The
# machine's mouse/keyboard must be idle during the run.
#
# DESTRUCTIVE: deletes config.json BESIDE THE EXE before launching (the
# TargetPath assertion needs a known-empty starting config - see below) and
# writes a fresh one via the Save Config button.
#
#   powershell -File scripts\gui-e2e.ps1 -Exe rust\target\debug\ProcDumpMonitor.exe -OutDir out\shots
#
# Exit codes: 0 = full walk completed, 1 = element/window not found or nav broken.
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$OutDir
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text;
namespace W {
  [StructLayout(LayoutKind.Sequential)] public struct RC { public int L,T,R,B; }
  [StructLayout(LayoutKind.Sequential)] public struct PT { public int X,Y; }
  [StructLayout(LayoutKind.Sequential)] public struct CBINFO {
    public int cbSize; public RC rcItem; public RC rcButton; public int stateButton;
    public IntPtr hwndCombo; public IntPtr hwndItem; public IntPtr hwndList; }
  public static class U32 {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int d, UIntPtr e);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool GetCursorPos(ref PT p);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(PT p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] public static extern int SendMessageStr(IntPtr hWnd, uint Msg, int wParam, string lParam);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, IntPtr lParam, uint flags, uint timeoutMs, out UIntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int SendMessageW(IntPtr hWnd, uint Msg, int wParam, StringBuilder lParam);
    [DllImport("user32.dll", EntryPoint="SendMessageW")] public static extern int SendMessageInt(IntPtr hWnd, uint Msg, int wParam, int lParam);
    [DllImport("user32.dll")] public static extern bool GetComboBoxInfo(IntPtr h, ref CBINFO i);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int idx);
    [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  }
}
'@

$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path

# Stale instances of the same exe would make the window lookup ambiguous - and a
# stale one still holding config.json open would defeat the delete below, which
# is what the whole TargetPath assertion rests on. WAIT for them to die.
Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -eq $Exe } | Stop-Process -Force -PassThru -ErrorAction SilentlyContinue |
    Wait-Process -Timeout 15 -ErrorAction SilentlyContinue

$script:retries = 0
function Fail([string]$msg) { Write-Host "FAIL: $msg"; try { if ($script:proc -and !$script:proc.HasExited) { $script:proc.Kill() } } catch {}; exit 1 }

# PRECONDITION: an unlocked, interactive desktop. This is the single biggest
# source of untrustworthy results. Injected input goes to whoever owns the
# input desktop; on a LOCKED workstation that is LockApp, so every click is
# silently swallowed - while UIA still enumerates the controls and window
# messages still answer, so the run LOOKS like it is working and fails
# somewhere arbitrary. Measured on 2026-07-26: locked, GetForegroundWindow()
# returned 0 and a synthesized click landed on 'Windows Default Lock Screen'.
# A locked session is the most likely explanation for the historical "0 -> 0"
# scroll false-RED and for the one lost SMTP click. One second to rule out.
function Assert-Interactive {
    for ($t = 0; $t -lt 8; $t++) {
        $h = [W.U32]::GetForegroundWindow()
        if ($h -ne [IntPtr]::Zero) {
            $fgPid = 0
            [W.U32]::GetWindowThreadProcessId($h, [ref]$fgPid) | Out-Null
            $n = try { (Get-Process -Id $fgPid -ErrorAction Stop).ProcessName } catch { '' }
            if ($n -and $n -notin @('LockApp', 'LogonUI')) { Write-Host "desktop: interactive (foreground = $n)"; return }
        }
        Start-Sleep -Milliseconds 250
    }
    Fail "workstation appears LOCKED (no foreground window, or LockApp/LogonUI owns it). Synthesized clicks would go to the lock screen, not the app. Unlock the session, leave the mouse and keyboard idle, and re-run."
}
Assert-Interactive

# The TargetPath assertion at the end of the walk needs a KNOWN-EMPTY starting
# config, and this MUST happen before Start-Process: gui/mod.rs reads
# config.json into memory once, at launch, so deleting it mid-walk leaves the
# in-memory copy (and a stale-but-correct TargetPath) intact and the assertion
# passes even with the feature broken. With no file, Config::load returns
# Default -> TargetName empty -> set_target() sees a real change and CLEARS
# TargetPath, so only capture_target_path() can put it back.
# config.json lives beside the EXE (paths::install_dir), never in OutDir.
$cfgPath = Join-Path (Split-Path $Exe) 'config.json'
Remove-Item $cfgPath -Force -ErrorAction SilentlyContinue
# VERIFY the delete. The assertion's entire discriminating power rests on this
# file being gone: if it survives (locked, or a stale instance still holding it)
# the app loads a config whose TargetName ALREADY equals the target we select,
# set_target() sees no change and does not clear, and resolve_target_path()'s
# Process arm trusts the existing target_path and writes it straight back -
# green run, broken feature. That self-poisoning is exactly what a review
# rejected once already, so it does not get to come back through a silent
# -ErrorAction SilentlyContinue.
if (Test-Path $cfgPath) { Fail "could not delete $cfgPath - the TargetPath assertion would be meaningless (file locked, or a stale instance still running?)" }
Write-Host "reset:  removed $cfgPath"

Write-Host "launch: $Exe"
$script:proc = Start-Process -FilePath $Exe -PassThru
$auto = [System.Windows.Automation.AutomationElement]

# Find the main window by PID (poll up to 15s).
$win = $null
for ($i = 0; $i -lt 60 -and -not $win; $i++) {
    Start-Sleep -Milliseconds 250
    if ($proc.HasExited) { Fail "process exited early (code $($proc.ExitCode)) - UAC-manifest build?" }
    $cond = New-Object System.Windows.Automation.PropertyCondition($auto::ProcessIdProperty, $proc.Id)
    $win = $auto::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}
if (-not $win) { Fail "main window not found within 15s" }
Write-Host "window: '$($win.Current.Name)'"
$script:hwnd = [IntPtr]$win.Current.NativeWindowHandle

# Snap the window to the work-area origin before driving it. Measured on
# 2026-07-26 (1920x1080 @ 125%): the window is 1022px tall, the work area is
# 1020px, and as LAUNCHED (centred, Y=53, bottom 1075) the ENTIRE footer row -
# Create Task / Run Now / Stop / Remove / Save Config / ... - sits UNDER THE
# TASKBAR. Every footer click landed on MSTaskSwWClass instead, and clicking
# the taskbar pops a full-screen topmost explorer XAML island that then
# swallows every later click, in this run and the next. At Y=0 the footer sits
# at y=990 and clicks land on the button. Not cosmetic: the TargetPath
# assertion needs the Save Config button.
#
# COORDINATE-SPACE CHECK FIRST. UIA BoundingRectangles are physical px, but
# Forms' WorkingArea is in THIS process's space, which Windows virtualizes when
# the host is DPI-unaware - 1920x1080 reads back as 1536x864 at 125%. Both
# spaces demonstrably exist on this box. If they ever disagree, the work-area
# guard in Click-El would reject EVERY footer click as out of bounds: a blanket
# false red, the exact failure class this script exists to eliminate. Compared
# on the VIRTUAL screen, which is what UIA's root element covers, so this stays
# correct on multi-monitor.
$vs = [System.Windows.Forms.SystemInformation]::VirtualScreen
$desk = $auto::RootElement.Current.BoundingRectangle
if ([Math]::Abs($desk.Width - $vs.Width) -gt 2 -or [Math]::Abs($desk.Height - $vs.Height) -gt 2) {
    Fail "HARNESS: coordinate-space mismatch - UIA sees a $([int]$desk.Width)x$([int]$desk.Height) desktop, this process sees $($vs.Width)x$($vs.Height) (DPI virtualization). Every screen-coordinate comparison would be wrong; run under a DPI-aware host."
}
$script:workArea = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
[W.U32]::SetWindowPos($script:hwnd, [IntPtr]::Zero, $script:workArea.X, $script:workArea.Y, 0, 0, 0x0005) | Out-Null  # NOSIZE|NOZORDER
Start-Sleep -Milliseconds 400
$wr = $win.Current.BoundingRectangle
Write-Host "window: $([int]$wr.Width)x$([int]$wr.Height) snapped to ($($script:workArea.X),$($script:workArea.Y)); work area $($script:workArea.Width)x$($script:workArea.Height)"
if ($wr.Height -gt $script:workArea.Height) {
    Write-Host "warn:  window is TALLER than the work area ($([int]$wr.Height) > $($script:workArea.Height)) - bottom-row controls may be unreachable on this display"
}
[W.U32]::SetForegroundWindow($script:hwnd) | Out-Null
Start-Sleep -Milliseconds 500

# --- element finders --------------------------------------------------------
function All-Els([object]$root) {
    $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
}
function Find-El([string]$class, [string]$namePattern, [object]$root = $win) {
    foreach ($el in All-Els $root) {
        if (-not $el.Current.IsOffscreen -and
            ($class -eq '' -or $el.Current.ClassName -ieq $class) -and
            $el.Current.Name -like $namePattern) { return $el }
    }
    return $null
}
# Sidebar nav labels share their text with content titles; disambiguate by the
# small WINDOW-RELATIVE X of the sidebar column (BoundingRectangle.X is an
# absolute screen coord, so subtract the window's left edge).
function Find-Nav([string]$name) {
    $winX = $win.Current.BoundingRectangle.X
    foreach ($el in All-Els $win) {
        # -ceq (case-SENSITIVE): the "MONITOR" group caption would otherwise
        # match "Monitor" under PowerShell's default case-insensitive -eq and
        # shadow the real (clickable) nav item.
        if ($el.Current.ClassName -ieq 'STATIC' -and $el.Current.Name -ceq $name -and
            -not $el.Current.IsOffscreen -and ($el.Current.BoundingRectangle.X - $winX) -lt 260) { return $el }
    }
    return $null
}
function Require([object]$el, [string]$what) { if (-not $el) { Fail "not found: $what" }; return $el }
# $fg = top-level window hwnd to bring forward before clicking. For a modal
# dialog this MUST be the dialog (the main window is disabled and would cover
# the dialog if forced forward). Defaults to the main window.
function Click-El([object]$el, [System.IntPtr]$fg = $script:hwnd) {
    [W.U32]::SetForegroundWindow($fg) | Out-Null
    $r = $el.Current.BoundingRectangle
    $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
    # NEVER click outside the work area. Out there is the taskbar, and a click
    # on it opens a full-screen topmost shell overlay that silently eats every
    # subsequent click - one unreachable control turns into a run-wide mystery
    # (and poisons the NEXT run too). Fail loudly instead.
    # NOT a HARNESS fault: an unreachable control is a real product-layout
    # defect (the window does not fit this display), so it must not be labelled
    # as a harness problem and waved away.
    if (-not $script:workArea.Contains($x, $y)) {
        Fail "LAYOUT: click target ($x,$y) for '$($el.Current.Name)' is outside the work area $($script:workArea) - the window does not fit this display, so a real user cannot reach this control either"
    }
    [W.U32]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 120
    [W.U32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)   # LEFTDOWN
    [W.U32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)   # LEFTUP
    Start-Sleep -Milliseconds 400
}
function Type-Text([object]$el, [string]$text) {
    Click-El $el
    [System.Windows.Forms.SendKeys]::SendWait("^a{DEL}")
    # Escape SendKeys metacharacters in file paths etc.
    $esc = $text -replace '([+^%~(){}\[\]])', '{$1}'
    [System.Windows.Forms.SendKeys]::SendWait($esc)
    Start-Sleep -Milliseconds 300
}
function Wait-Idle {
    $res = [System.UIntPtr]::Zero
    [W.U32]::SendMessageTimeout($script:hwnd, 0x0000, [UIntPtr]::Zero, [IntPtr]::Zero, 0x0002, 5000, [ref]$res) | Out-Null
}
# CB_GETCOUNT (0x0146): deterministic item count of a combo, regardless of
# whether it's dropped open (a closed combo always LOOKS empty in a screenshot).
function Combo-Count([object]$combo) {
    $res = [System.UIntPtr]::Zero
    $h = [IntPtr]$combo.Current.NativeWindowHandle
    [W.U32]::SendMessageTimeout($h, 0x0146, [UIntPtr]::Zero, [IntPtr]::Zero, 0x0002, 3000, [ref]$res) | Out-Null
    return [int]$res.ToUInt32()
}
# Topmost visible combobox on the current page (the target combo sits above the
# scenario / dump-type combos).
function Top-Combo {
    $best = $null; $bestY = [double]::MaxValue
    foreach ($el in All-Els $win) {
        if ($el.Current.ClassName -ieq 'COMBOBOX' -and -not $el.Current.IsOffscreen) {
            $y = $el.Current.BoundingRectangle.Y
            if ($y -lt $bestY) { $bestY = $y; $best = $el }
        }
    }
    return $best
}
function Shot([string]$name, [object]$target = $win) {
    [W.U32]::SetForegroundWindow([IntPtr]$target.Current.NativeWindowHandle) | Out-Null
    Wait-Idle
    Start-Sleep -Milliseconds 250
    $r = $target.Current.BoundingRectangle
    $bmp = New-Object System.Drawing.Bitmap([int]$r.Width, [int]$r.Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, $bmp.Size)
    $g.Dispose()
    $path = Join-Path $OutDir "$name.png"
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    Write-Host "shot:  $path"
}
# Find an owned dialog window (Advanced / SMTP) by title. Owned windows nest
# UNDER their owner in the UIA tree, so search descendants, not root children.
# Visibility filter: both dialogs are OWNED and REUSABLE - nwg HIDES them on
# close instead of destroying them - so a hidden one that is still in the UIA
# tree would otherwise be handed back as though it had opened. Filtered on
# IsWindowVisible, not UIA IsOffscreen: IsOffscreen means "clipped out of view"
# and stays FALSE for a hidden window, so it does not answer this question.
function Find-Dialog([string]$title, [int]$tries = 40) {
    $pidCond = New-Object System.Windows.Automation.PropertyCondition($auto::ProcessIdProperty, $proc.Id)
    $winCond = New-Object System.Windows.Automation.PropertyCondition($auto::ControlTypeProperty, [System.Windows.Automation.ControlType]::Window)
    $and = New-Object System.Windows.Automation.AndCondition($pidCond, $winCond)
    $seen = @()
    for ($t = 0; $t -lt $tries; $t++) {
        $seen = @()
        foreach ($w in $auto::RootElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $and)) {
            $vis = [W.U32]::IsWindowVisible([IntPtr]$w.Current.NativeWindowHandle)
            $off = ''; if (-not $vis) { $off = ' [hidden]' }
            $seen += "'$($w.Current.Name)'$off"
            if ($w.Current.Name -eq $title -and $vis) { return $w }
        }
        Start-Sleep -Milliseconds 150
    }
    # Leave evidence: the SMTP step failed here once with no way to tell whether
    # the click was lost or the dialog opened under a different title.
    Write-Host "probe: dialog '$title' not found; process windows seen: $($seen -join ', ')"
    return $null
}
# Click a button that opens an owned dialog, then wait for the dialog.
# The single re-click papers over a LOST FIRST CLICK - nothing else. It does
# not cover a dialog that fails to build (the second attempt would miss too).
# See the Task 8 report: the SMTP step failed once on 2026-07-26 and was green
# on an immediate re-run; the cause was never reproduced. Every retry is
# printed, so a run that needed one is visible in the transcript.
function Open-Dialog([object]$btn, [string]$title) {
    Click-El $btn; Start-Sleep -Milliseconds 500
    $d = Find-Dialog $title
    if (-not $d) {
        $script:retries++
        Write-Host "retry: '$title' did not appear after the first click - clicking once more"
        Click-El $btn; Start-Sleep -Milliseconds 500
        $d = Find-Dialog $title
    }
    return $d
}
# A fixed sleep after Save & Close let the NEXT click land while the dialog was
# still up (a modal dialog covers the footer buttons). Wait for it to actually go.
# IsWindowVisible, NOT UIA's IsOffscreen: IsOffscreen means "clipped out of
# view", not "hidden", and it stayed FALSE for a dialog nwg had already hidden
# (measured 2026-07-26 - it turned a passing step into a 4s hard failure).
# IsWindowVisible is the authoritative answer and it is what nwg actually flips.
function Wait-DialogGone([System.IntPtr]$hwnd, [string]$title) {
    for ($t = 0; $t -lt 40; $t++) {
        if (-not [W.U32]::IsWindowVisible($hwnd)) { return }
        Start-Sleep -Milliseconds 100
    }
    Fail "dialog '$title' (hwnd $hwnd) still VISIBLE 4s after Save & Close - the close click did not land"
}

# =====================  Monitor page  =====================
Shot '01-monitor'

# Deterministic: the combined target dropdown is populated (processes +
# running services).
$combo = Require (Top-Combo) "target combo"
$comboH = [IntPtr]$combo.Current.NativeWindowHandle
$c0 = Combo-Count $combo
Write-Host "probe: target combo has $c0 entries (hint row + processes + running services)"
# 2, not 1: the hint row is UNCONDITIONAL since Task 7, so "-lt 1" could never
# fire. A list holding nothing but the hint row is an empty list.
if ($c0 -lt 2) { Fail "target combo holds only the hint row ($c0 entries; expected processes and services)" }

# CB_GETLBTEXT = 0x0148. Index 0 is the hint row.
$sb = New-Object System.Text.StringBuilder 512
[W.U32]::SendMessageW($comboH, 0x0148, 0, $sb) | Out-Null
$first = $sb.ToString()
Write-Host "probe: first entry = '$first'"
if ($first -ne '- Select a process or service -') { Fail "first target entry is not the hint row: '$first'" }

# PROCESSES MUST COME FIRST (they'd be unreachable under 150+ services). Since
# Task 7 index 0 is the hint, so the ordering check reads index 1 - the first
# REAL entry. A FRESH StringBuilder: CB_GETLBTEXT returns CB_ERR (-1) on an
# out-of-range index and leaves the buffer untouched, so a reused $sb would
# read back the hint text and pass.
$sb1 = New-Object System.Text.StringBuilder 512
if ([W.U32]::SendMessageW($comboH, 0x0148, 1, $sb1) -lt 0) { Fail "target combo has no entry at index 1" }
$second = $sb1.ToString()
Write-Host "probe: second entry = '$second'"
if ($second -notlike 'Proc: *') { Fail "index 1 is not a process ('$second') - services must not precede processes" }

# The dropdown must be SCROLLABLE BY THE USER. nwg omits WS_VSCROLL (its
# ComboBoxFlags has no scroll bit), which leaves the drop-down list capped at
# the ~30 rows Windows shows by default with no way to reach the rest.
# NOTE: CB_SETTOPINDEX is NOT a valid test -- it repositions the list even when
# there is no scrollbar and the wheel is dead (that false-green shipped once).
# Drop the list, put the real cursor over it, and send real wheel input.
#
# mouse_event's wheel goes to the FOREGROUND window, and SetForegroundWindow is
# REFUSED to a caller that does not own the foreground - so a lost activation
# looks exactly like a dead scroll (that false RED happened on 2026-07-26:
# "0 -> 0" on a correct build, "0 -> 45" on an identical re-run). Check it
# HERE, before the list drops, where the answer is unambiguous, and fail with a
# HARNESS message so a broken harness never reads as a product bug.
[W.U32]::SetForegroundWindow($script:hwnd) | Out-Null
Start-Sleep -Milliseconds 250
$fg = [W.U32]::GetForegroundWindow()
Write-Host "probe: foreground=$fg app=$($script:hwnd)"
if ($fg -ne $script:hwnd) { Fail "HARNESS: could not foreground the app window (fg=$fg app=$($script:hwnd)) - the wheel would go elsewhere, so a scroll result here is not trustworthy" }
[W.U32]::SendMessageInt($comboH, 0x014F, 1, 0) | Out-Null   # CB_SHOWDROPDOWN
Start-Sleep -Milliseconds 500
$cbi = New-Object W.CBINFO; $cbi.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($cbi)
[W.U32]::GetComboBoxInfo($comboH, [ref]$cbi) | Out-Null
$style = [W.U32]::GetWindowLong($cbi.hwndList, -16)
if (-not ($style -band 0x00200000)) { Fail "target dropdown list has no WS_VSCROLL (unreachable items)" }
$lb = New-Object W.RC; [W.U32]::GetWindowRect($cbi.hwndList, [ref]$lb) | Out-Null
[W.U32]::SetCursorPos([int](($lb.L + $lb.R) / 2), [int](($lb.T + $lb.B) / 2)) | Out-Null
Start-Sleep -Milliseconds 200
# Second half of the same guard: the wheel hits the window under the CURSOR.
$pt = New-Object W.PT; [W.U32]::GetCursorPos([ref]$pt) | Out-Null
$under = [W.U32]::WindowFromPoint($pt)
if ($under -ne $cbi.hwndList) { Fail "HARNESS: cursor ($($pt.X),$($pt.Y)) is not over the dropdown list (under=$under list=$($cbi.hwndList)) - the scroll result is not trustworthy" }
# Re-check: ~1s elapsed while the list dropped, and activation can be lost in
# it. The wheel is about to fire NOW, so this is the reading that matters.
$fg2 = [W.U32]::GetForegroundWindow()
if ($fg2 -ne $script:hwnd) { Fail "HARNESS: lost the foreground while dropping the list (fg=$fg2 app=$($script:hwnd)) - the scroll result is not trustworthy" }
$topBefore = [W.U32]::SendMessageInt($comboH, 0x015B, 0, 0)   # CB_GETTOPINDEX
for ($w = 0; $w -lt 5; $w++) { [W.U32]::mouse_event(0x0800, 0, 0, -360, [UIntPtr]::Zero); Start-Sleep -Milliseconds 120 }
Start-Sleep -Milliseconds 250
$topAfter = [W.U32]::SendMessageInt($comboH, 0x015B, 0, 0)
Write-Host "probe: real mouse wheel scrolled dropdown $topBefore -> $topAfter"
if ($topAfter -le $topBefore) { Fail "mouse wheel did not scroll the target dropdown" }
[W.U32]::SendMessageInt($comboH, 0x014F, 0, 0) | Out-Null    # close the list
Start-Sleep -Milliseconds 200

# "Include stopped services" must ADD entries. Adjudicated by count, not pixels.
$showall = Require (Find-El 'BUTTON' '*stopped services*') "show-all checkbox"
Click-El $showall; Wait-Idle
$c1 = Combo-Count $combo
Write-Host "probe: with stopped services, target combo has $c1 entries"
if ($c1 -le $c0) { Fail "show-all did not grow the target list ($c0 -> $c1)" }
Shot '02-monitor-showall'

# Refresh keeps it populated.
$refresh = Require (Find-El 'BUTTON' 'Refresh') "refresh button"
Click-El $refresh; Wait-Idle
if ((Combo-Count $combo) -lt 2) { Fail "target combo holds only the hint row after refresh" }
Write-Host "click: refresh services"

# Pick a known target so the TargetPath assertion at the end of the walk has a
# subject: the app UNDER TEST itself - certainly running, certainly a Process
# target, and its image path is known exactly ($Exe). CB_SETCURSEL is enough,
# no click needed: MonitorPage::save() reads the pick through nwg's
# ComboBox::selection(), which is a live CB_GETCURSEL. The script has no
# find/select helper, hence the raw messages.
$wantLabel = "Proc: $([IO.Path]::GetFileName($Exe))"
$idx = [W.U32]::SendMessageStr($comboH, 0x0158, -1, $wantLabel)   # CB_FINDSTRINGEXACT
if ($idx -lt 0) { Fail "target combo has no entry '$wantLabel' (the app under test should be listed as a running process)" }
[W.U32]::SendMessageInt($comboH, 0x014E, $idx, 0) | Out-Null      # CB_SETCURSEL
$cur = [W.U32]::SendMessageInt($comboH, 0x0147, 0, 0)             # CB_GETCURSEL
if ($cur -ne $idx) { Fail "could not select '$wantLabel' (CB_SETCURSEL left the pick at $cur, wanted $idx)" }
Write-Host "select: target -> '$wantLabel' (index $idx)"

# Live status rows are present on the Monitor page. Match the status glyphs
# (the subtitle also contains "scheduled task", so key off the leading marker).
function Find-Status {
    foreach ($el in All-Els $win) {
        if ($el.Current.ClassName -ieq 'STATIC' -and -not $el.Current.IsOffscreen -and
            ($el.Current.Name -like "$([char]0x2713)*" -or $el.Current.Name -like "$([char]0x25CB)*") -and
            $el.Current.Name -like '*cheduled task*') { return $el }
    }
    return $null
}
$stTask = Require (Find-Status) "status: task row"
Write-Host "status: '$($stTask.Current.Name)'"

# --- Advanced dialog: open, screenshot, close ---
$advBtn = Require (Find-El 'BUTTON' 'Advanced*') "Advanced button"
$adv = Require (Open-Dialog $advBtn 'Advanced options') "Advanced dialog"
$advHwnd = [IntPtr]$adv.Current.NativeWindowHandle
Shot '03-advanced-dialog' $adv
$advClose = Require (Find-El 'BUTTON' 'Save*Close*' $adv) "Advanced close button"
Click-El $advClose $advHwnd
Wait-DialogGone $advHwnd 'Advanced options'
if (-not $win.Current.IsEnabled) { Fail "main window still disabled after closing Advanced dialog" }
Write-Host "dialog: advanced opened + closed"

# --- SMTP dialog: open, screenshot, close ---
$smtpBtn = Require (Find-El 'BUTTON' 'SMTP*') "SMTP button"
$smtp = Require (Open-Dialog $smtpBtn 'SMTP settings') "SMTP dialog"
$smtpHwnd = [IntPtr]$smtp.Current.NativeWindowHandle
Shot '04-smtp-dialog' $smtp
$smtpClose = Require (Find-El 'BUTTON' 'Save*Close*' $smtp) "SMTP close button"
Click-El $smtpClose $smtpHwnd
Wait-DialogGone $smtpHwnd 'SMTP settings'
if (-not $win.Current.IsEnabled) { Fail "main window still disabled after closing SMTP dialog" }
Write-Host "dialog: smtp opened + closed"

# =====================  Collector pages  =====================
$dc = Require (Find-Nav 'Data Collection') "nav: Data Collection"
Click-El $dc; Wait-Idle; Shot '05-datacollection'
Write-Host "nav:   Data Collection"

$il = Require (Find-Nav 'Install Logs') "nav: Install Logs"
Click-El $il; Wait-Idle; Shot '06-installlogs'
Write-Host "nav:   Install Logs"

$sh = Require (Find-Nav 'System Health') "nav: System Health"
Click-El $sh; Wait-Idle; Shot '07-systemhealth'
Write-Host "nav:   System Health"

# Run a REAL System Health collection into OutDir and wait for it to finish.
# The save-path box is the LOWEST edit on the page (below the two pattern
# boxes); pick the max-Y onscreen edit deterministically.
$saveBox = $null; $maxY = -1
foreach ($el in All-Els $win) {
    if ($el.Current.ClassName -ieq 'EDIT' -and -not $el.Current.IsOffscreen) {
        $y = $el.Current.BoundingRectangle.Y
        if ($y -gt $maxY) { $maxY = $y; $saveBox = $el }
    }
}
if (-not $saveBox) { Fail "save-path box not found on System Health page" }
Type-Text $saveBox $OutDir; Write-Host "type:  save path -> OutDir"
$collectBtn = Require (Find-El 'BUTTON' 'Collect system health') "collect button"
Click-El $collectBtn; Write-Host "click: collect system health"

# Poll the status label until Done/FAILED (or 60s). Text via UIA Name.
$done = $false
for ($i = 0; $i -lt 120 -and -not $done; $i++) {
    Start-Sleep -Milliseconds 500
    $st = Find-El 'STATIC' 'Status:*'
    if ($st -and ($st.Current.Name -like '*Done*' -or $st.Current.Name -like '*FAILED*')) {
        Write-Host "collect: $($st.Current.Name)"
        if ($st.Current.Name -like '*FAILED*') { Fail "system health collection reported FAILED" }
        $done = $true
    }
}
if (-not $done) { Fail "system health collection did not finish within 60s" }
Shot '08-systemhealth-done'

$about = Require (Find-Nav 'About') "nav: About"
Click-El $about; Wait-Idle; Shot '09-about'
Write-Host "nav:   About"

# Back to Monitor: status panel must still render.
$mon = Require (Find-Nav 'Monitor') "nav: Monitor"
Click-El $mon; Wait-Idle; Shot '10-back-monitor'
if (-not (Find-Status)) { Fail "status panel missing after returning to Monitor" }
Write-Host "nav:   Monitor (status panel intact)"

# =====================  TargetPath survives a real save  =====================
# The ONLY possible coverage for bitness::capture_target_path's single call
# site (page_monitor.rs, MonitorPage::save): comment that one line out and the
# whole Rust suite stays green while TargetPath never populates and bitness
# silently degrades. No unit test can reach save() - it needs live nwg controls.
#
# Save Config (and Create Task) are the only buttons that WRITE config.json;
# save() alone is in-memory. The footer is hidden on every LOG COLLECTOR page
# (gui/mod.rs does set_visible(next == 0) for the whole row), so this has to run
# with the Monitor page showing - hence the nav back above.
$saveCfg = Require (Find-El 'BUTTON' 'Save Config') "Save Config button"
Click-El $saveCfg; Wait-Idle; Start-Sleep -Milliseconds 500
if (-not (Test-Path $cfgPath)) { Fail "Save Config wrote no config: $cfgPath" }
$cfgJson = Get-Content $cfgPath -Raw | ConvertFrom-Json
if ($cfgJson.PSObject.Properties.Name -notcontains 'TargetPath') { Fail "config.json has no TargetPath key" }
$tp = [string]$cfgJson.TargetPath
Write-Host "probe: TargetName='$($cfgJson.TargetName)' TargetPath='$tp'"
# NON-EMPTY is the assertion that kills the mutant: set_target() cleared the
# field (config.json was deleted before launch, so the saved name started
# empty), and capture_target_path() is the only thing that can refill it.
if ([string]::IsNullOrWhiteSpace($tp)) { Fail "TargetPath is EMPTY - bitness::capture_target_path() never ran (call site removed from MonitorPage::save?)" }
if (-not (Test-Path $tp)) { Fail "TargetPath does not exist on disk: '$tp'" }
$wantImage = [IO.Path]::GetFileName($Exe)
if ([IO.Path]::GetFileName($tp) -ine $wantImage) { Fail "TargetPath names the wrong image: '$tp' (selected target was '$wantImage')" }
Write-Host "probe: TargetPath captured, exists on disk, matches the selected target"
Shot '11-saved-config'

# =====================  Capture logs  =====================
$logDst = Join-Path $OutDir 'logs'
New-Item -ItemType Directory -Force $logDst | Out-Null
$appLog = Join-Path (Split-Path $Exe) 'Logs\procdump.log'
if (Test-Path $appLog) { Copy-Item $appLog $logDst -Force; Write-Host "log:   copied app log" }
# Newest run folder produced by the collection (OutDir\YYYY-MM-DD\Run_*).
$run = Get-ChildItem -Path $OutDir -Directory -Recurse -Filter 'Run_*' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($run) {
    Copy-Item (Join-Path $run.FullName 'Run_Transcript.txt') $logDst -Force -ErrorAction SilentlyContinue
    Copy-Item (Join-Path $run.FullName 'Collection_Summary.txt') $logDst -Force -ErrorAction SilentlyContinue
    Write-Host "log:   captured collection transcript + summary from $($run.Name)"
} else {
    Write-Host "warn:  no collection run folder found under OutDir"
}

Write-Host "close: app"
if (-not $proc.HasExited) { $proc.CloseMainWindow() | Out-Null; Start-Sleep -Milliseconds 800 }
if (-not $proc.HasExited) { $proc.Kill() }
# A consumed retry means the PRODUCT worked and the HARNESS stuttered, so it
# does not fail the run - that would re-create the harness-fault-reads-as-
# product-bug confusion this script exists to remove. But a scrolled-past
# "retry:" line is easy to miss, so it is restated where nobody can miss it.
if ($script:retries -gt 0) { Write-Host "warn:  $($script:retries) dialog retry(ies) consumed - a first click was lost; investigate before trusting this as a clean run" }
Write-Host "OK: e2e walk complete, screenshots + logs in $OutDir"
exit 0

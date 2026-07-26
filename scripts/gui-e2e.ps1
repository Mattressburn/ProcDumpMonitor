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
  [StructLayout(LayoutKind.Sequential)] public struct CBINFO {
    public int cbSize; public RC rcItem; public RC rcButton; public int stateButton;
    public IntPtr hwndCombo; public IntPtr hwndItem; public IntPtr hwndList; }
  public static class U32 {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int d, UIntPtr e);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, IntPtr lParam, uint flags, uint timeoutMs, out UIntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int SendMessageW(IntPtr hWnd, uint Msg, int wParam, StringBuilder lParam);
    [DllImport("user32.dll", EntryPoint="SendMessageW")] public static extern int SendMessageInt(IntPtr hWnd, uint Msg, int wParam, int lParam);
    [DllImport("user32.dll")] public static extern bool GetComboBoxInfo(IntPtr h, ref CBINFO i);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int idx);
  }
}
'@

$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path

# Stale instances of the same exe would make the window lookup ambiguous.
Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -eq $Exe } | Stop-Process -Force -ErrorAction SilentlyContinue

function Fail([string]$msg) { Write-Host "FAIL: $msg"; try { if ($script:proc -and !$script:proc.HasExited) { $script:proc.Kill() } } catch {}; exit 1 }

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
function Find-Dialog([string]$title) {
    $pidCond = New-Object System.Windows.Automation.PropertyCondition($auto::ProcessIdProperty, $proc.Id)
    $winCond = New-Object System.Windows.Automation.PropertyCondition($auto::ControlTypeProperty, [System.Windows.Automation.ControlType]::Window)
    $and = New-Object System.Windows.Automation.AndCondition($pidCond, $winCond)
    for ($t = 0; $t -lt 20; $t++) {
        foreach ($w in $auto::RootElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $and)) {
            if ($w.Current.Name -eq $title) { return $w }
        }
        Start-Sleep -Milliseconds 150
    }
    return $null
}

# =====================  Monitor page  =====================
Shot '01-monitor'

# Deterministic: the combined target dropdown is populated (processes +
# running services).
$combo = Require (Top-Combo) "target combo"
$comboH = [IntPtr]$combo.Current.NativeWindowHandle
$c0 = Combo-Count $combo
Write-Host "probe: target combo has $c0 process + running-service entries"
if ($c0 -lt 1) { Fail "target combo empty (expected processes and services)" }

# PROCESSES MUST COME FIRST (they'd be unreachable under 150+ services).
# CB_GETLBTEXT = 0x0148.
$sb = New-Object System.Text.StringBuilder 512
[W.U32]::SendMessageW($comboH, 0x0148, 0, $sb) | Out-Null
$first = $sb.ToString()
Write-Host "probe: first entry = '$first'"
if ($first -ne '- Select a process or service -') { Fail "first target entry is not the hint row: '$first'" }

# The dropdown must be SCROLLABLE BY THE USER. nwg omits WS_VSCROLL (its
# ComboBoxFlags has no scroll bit), which leaves the drop-down list capped at
# the ~30 rows Windows shows by default with no way to reach the rest.
# NOTE: CB_SETTOPINDEX is NOT a valid test -- it repositions the list even when
# there is no scrollbar and the wheel is dead (that false-green shipped once).
# Drop the list, put the real cursor over it, and send real wheel input.
[W.U32]::SendMessageInt($comboH, 0x014F, 1, 0) | Out-Null   # CB_SHOWDROPDOWN
Start-Sleep -Milliseconds 500
$cbi = New-Object W.CBINFO; $cbi.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($cbi)
[W.U32]::GetComboBoxInfo($comboH, [ref]$cbi) | Out-Null
$style = [W.U32]::GetWindowLong($cbi.hwndList, -16)
if (-not ($style -band 0x00200000)) { Fail "target dropdown list has no WS_VSCROLL (unreachable items)" }
$lb = New-Object W.RC; [W.U32]::GetWindowRect($cbi.hwndList, [ref]$lb) | Out-Null
[W.U32]::SetCursorPos([int](($lb.L + $lb.R) / 2), [int](($lb.T + $lb.B) / 2)) | Out-Null
Start-Sleep -Milliseconds 200
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
if ((Combo-Count $combo) -lt 1) { Fail "target combo empty after refresh" }
Write-Host "click: refresh services"

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
Click-El $advBtn; Start-Sleep -Milliseconds 500
$adv = Require (Find-Dialog 'Advanced options') "Advanced dialog"
$advHwnd = [IntPtr]$adv.Current.NativeWindowHandle
Shot '03-advanced-dialog' $adv
$advClose = Require (Find-El 'BUTTON' 'Save*Close*' $adv) "Advanced close button"
Click-El $advClose $advHwnd; Start-Sleep -Milliseconds 400
if (-not $win.Current.IsEnabled) { Fail "main window still disabled after closing Advanced dialog" }
Write-Host "dialog: advanced opened + closed"

# --- SMTP dialog: open, screenshot, close ---
$smtpBtn = Require (Find-El 'BUTTON' 'SMTP*') "SMTP button"
Click-El $smtpBtn; Start-Sleep -Milliseconds 500
$smtp = Require (Find-Dialog 'SMTP settings') "SMTP dialog"
$smtpHwnd = [IntPtr]$smtp.Current.NativeWindowHandle
Shot '04-smtp-dialog' $smtp
$smtpClose = Require (Find-El 'BUTTON' 'Save*Close*' $smtp) "SMTP close button"
Click-El $smtpClose $smtpHwnd; Start-Sleep -Milliseconds 400
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
Write-Host "OK: e2e walk complete, screenshots + logs in $OutDir"
exit 0

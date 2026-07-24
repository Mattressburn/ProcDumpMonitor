# GUI end-to-end driver for ProcDump Monitor (Rust build).
# Launches the exe (build it with PDM_TEST_MANIFEST=1 so no UAC prompt),
# walks the wizard like a real user - physical mouse clicks + keystrokes -
# screenshotting every state. nwg controls expose no UIA patterns, so UIA is
# used only to FIND elements (class+name+rect); input is synthesized.
# Run under Windows PowerShell 5.1 (powershell.exe) for UIA assemblies.
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
Add-Type -Namespace W -Name U32 -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, System.UIntPtr e);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h);
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
$hwnd = [IntPtr]$win.Current.NativeWindowHandle
[W.U32]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 500

function Find-El([string]$class, [string]$namePattern) {
    # Match by Win32 class name + window text; skip hidden wizard pages' controls.
    foreach ($el in $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
        if (-not $el.Current.IsOffscreen -and
            ($class -eq '' -or $el.Current.ClassName -eq $class) -and
            $el.Current.Name -like $namePattern) { return $el }
    }
    return $null
}
function Require([object]$el, [string]$what) { if (-not $el) { Fail "not found: $what" }; return $el }
function Click-El([object]$el) {
    [W.U32]::SetForegroundWindow($hwnd) | Out-Null
    $r = $el.Current.BoundingRectangle
    $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
    [W.U32]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 120
    [W.U32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)   # LEFTDOWN
    [W.U32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)   # LEFTUP
    Start-Sleep -Milliseconds 400
}
function Type-Text([object]$el, [string]$text) {
    Click-El $el
    [System.Windows.Forms.SendKeys]::SendWait("^a{DEL}$text")
    Start-Sleep -Milliseconds 300
}
function Shot([string]$name) {
    Start-Sleep -Milliseconds 300
    $r = $win.Current.BoundingRectangle
    $bmp = New-Object System.Drawing.Bitmap([int]$r.Width, [int]$r.Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, $bmp.Size)
    $g.Dispose()
    $path = Join-Path $OutDir "$name.png"
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    Write-Host "shot:  $path"
}

$pages = @('target','procdump','task','notify','review','about')
$next  = Require (Find-El 'Button' 'Next*')  "Next button"
$back  = Require (Find-El 'Button' '*Back*') "Back button"

# --- Page 1: interact like a user ---
Shot '01-target'
$edit = Require (Find-El 'Edit' '*') "process-name edit"
Type-Text $edit 'notepad'; Write-Host "type:  'notepad' into process name"
$show = Find-El 'Button' '*Show all*'
if ($show) { Click-El $show; Write-Host "click: show-all checkbox" } else { Write-Host "warn:  show-all checkbox not found" }
$refresh = Find-El 'Button' '*efresh*'
if ($refresh) { Click-El $refresh; Write-Host "click: refresh services"; Start-Sleep -Milliseconds 800 } else { Write-Host "warn:  refresh button not found" }
Shot '01-target-filled'
if ($back.Current.IsEnabled) { Fail "Back enabled on page 1" }

# --- Forward walk through all pages ---
for ($p = 1; $p -lt $pages.Count; $p++) {
    if (-not $next.Current.IsEnabled) { Fail "Next disabled before reaching page $($p+1)" }
    Click-El $next; Write-Host "click: Next -> $($pages[$p])"
    Start-Sleep -Milliseconds 400
    Shot ('{0:d2}-{1}' -f ($p + 1), $pages[$p])
}
if ($next.Current.IsEnabled) { Write-Host "warn:  Next still enabled on last page" }

# --- Reverse walk back to page 1 ---
for ($p = $pages.Count - 2; $p -ge 0; $p--) {
    if (-not $back.Current.IsEnabled) { Fail "Back disabled during reverse walk at $($pages[$p])" }
    Click-El $back; Write-Host "click: Back -> $($pages[$p])"
}
Shot '07-back-target'
if ($back.Current.IsEnabled) { Fail "Back enabled after returning to page 1" }

Write-Host "close: app"
if (-not $proc.HasExited) { $proc.CloseMainWindow() | Out-Null; Start-Sleep -Milliseconds 800 }
if (-not $proc.HasExited) { $proc.Kill() }
Write-Host "OK: e2e walk complete, screenshots in $OutDir"
exit 0

<#
.SYNOPSIS
  Capture the screenshots used by docs/customer/*.md (the branded PDF guides).

.DESCRIPTION
  Run under powershell.exe 5.1 (UIA assemblies), against a PDM_TEST_MANIFEST=1
  debug build so no UAC prompt interrupts the capture.

  Opens the preset list with CB_SHOWDROPDOWN rather than a click, but sidebar
  navigation DOES move the mouse: nwg statics expose no UIA patterns (everything
  is a Pane), so there is nothing to Invoke and a real click is the only way to
  change pages. Keep the machine idle for the few seconds a run takes, and keep
  the session unlocked - CopyFromScreen captures the lock screen otherwise.

.EXAMPLE
  powershell.exe -File scripts\guide-shots.ps1 `
      -Exe rust\target\debug\LogDump.exe -OutDir docs\customer\img
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$OutDir
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing, System.Windows.Forms

Add-Type @"
using System;
using System.Runtime.InteropServices;
namespace G {
  [StructLayout(LayoutKind.Sequential)] public struct RC { public int L,T,R,B; }
  [StructLayout(LayoutKind.Sequential)] public struct CBINFO {
    public int cbSize; public RC rcItem; public RC rcButton; public int stateButton;
    public IntPtr hwndCombo; public IntPtr hwndItem; public IntPtr hwndList; }
  public static class U {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
    [DllImport("user32.dll")] public static extern bool GetComboBoxInfo(IntPtr h, ref CBINFO i);
    [DllImport("user32.dll", EntryPoint="SendMessageW")] public static extern int SendMsg(IntPtr h, uint m, int w, int l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, UIntPtr w, IntPtr l, uint f, uint t, out UIntPtr r);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int d, UIntPtr e);
  }
}
"@

$CB_SHOWDROPDOWN = 0x014F

if (-not (Test-Path $Exe)) { throw "exe not found: $Exe" }
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path

function Wait-Idle {
    $r = [System.UIntPtr]::Zero
    [G.U]::SendMessageTimeout($script:hwnd, 0x0000, [UIntPtr]::Zero, [IntPtr]::Zero, 0x0002, 5000, [ref]$r) | Out-Null
    Start-Sleep -Milliseconds 220
}
function All-Els($root) {
    $root.FindAll([System.Windows.Automation.TreeScope]::Descendants,
                  [System.Windows.Automation.Condition]::TrueCondition)
}
# Sidebar nav rows share their text with the page title; the sidebar column is
# narrow, so filter on a small WINDOW-RELATIVE x. BoundingRectangle.X is an
# ABSOLUTE screen coord - subtract the window's left edge or every element matches.
function Find-Nav([string]$name) {
    $winX = $win.Current.BoundingRectangle.X
    foreach ($el in All-Els $win) {
        if ($el.Current.ClassName -ieq 'STATIC' -and $el.Current.Name -ceq $name -and
            -not $el.Current.IsOffscreen -and
            ($el.Current.BoundingRectangle.X - $winX) -lt 260) { return $el }
    }
    $null
}
function Click-El($el) {
    [G.U]::SetForegroundWindow($script:hwnd) | Out-Null
    $r = $el.Current.BoundingRectangle
    $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
    [G.U]::SetCursorPos($x, $y) | Out-Null
    Start-Sleep -Milliseconds 120
    [G.U]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)   # LEFTDOWN
    [G.U]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)   # LEFTUP
    Wait-Idle
}
function Grab([string]$name, [int]$x, [int]$y, [int]$w, [int]$h) {
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size)
    $g.Dispose()
    $p = Join-Path $OutDir "$name.png"
    $bmp.Save($p, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "shot: $name.png  ${w}x${h}"
}
function Shot-Window([string]$name) {
    [G.U]::SetForegroundWindow($script:hwnd) | Out-Null
    Wait-Idle
    $r = $win.Current.BoundingRectangle
    Grab $name ([int]$r.X) ([int]$r.Y) ([int]$r.Width) ([int]$r.Height)
}

# --- launch -----------------------------------------------------------------
$proc = Start-Process -FilePath (Resolve-Path $Exe) -PassThru
try {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $proc.Id)
    $win = $null
    foreach ($i in 1..40) {
        Start-Sleep -Milliseconds 400
        $win = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
        if ($win) { break }
    }
    if (-not $win) { throw "window never appeared" }
    $script:hwnd = [IntPtr]$win.Current.NativeWindowHandle

    # Pin to the work area's top-left so the whole window is on-screen and no
    # part of it falls under the taskbar (which would be captured as taskbar).
    $wa = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    [G.U]::SetWindowPos($script:hwnd, [IntPtr]::Zero, $wa.X, $wa.Y, 0, 0, 0x0005) | Out-Null
    Start-Sleep -Milliseconds 500
    [G.U]::SetForegroundWindow($script:hwnd) | Out-Null
    Start-Sleep -Milliseconds 600

    $wr = $win.Current.BoundingRectangle
    Write-Host "window: $([int]$wr.Width)x$([int]$wr.Height) at ($([int]$wr.X),$([int]$wr.Y)); work area $($wa.Width)x$($wa.Height)"
    if ($wr.Height -gt $wa.Height) {
        Write-Host "warn: window taller than work area - bottom row may be clipped in shots"
    }

    # 1. the ProcDump page as it opens
    Shot-Window '01-procdump-page'

    # 2. preset list open. The dropdown is its own popup window that extends
    #    BELOW the main window, so capture the union of both rects or the list
    #    gets clipped off the bottom.
    $combos = @(All-Els $win | Where-Object { $_.Current.ClassName -ieq 'ComboBox' })
    Write-Host "combos found: $($combos.Count)"
    if ($combos.Count -ge 2) {
        $preset = $combos[1]          # 0 = target picker, 1 = preset
        $ch = [IntPtr]$preset.Current.NativeWindowHandle
        [G.U]::SendMsg($ch, $CB_SHOWDROPDOWN, 1, 0) | Out-Null
        Start-Sleep -Milliseconds 700
        $info = New-Object G.CBINFO
        $info.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($info)
        [G.U]::GetComboBoxInfo($ch, [ref]$info) | Out-Null
        $lr = New-Object G.RC
        [G.U]::GetWindowRect($info.hwndList, [ref]$lr) | Out-Null
        $x1 = [Math]::Min([int]$wr.X, $lr.L); $y1 = [Math]::Min([int]$wr.Y, $lr.T)
        $x2 = [Math]::Max([int]($wr.X + $wr.Width), $lr.R)
        $y2 = [Math]::Max([int]($wr.Y + $wr.Height), $lr.B)
        Grab '02-preset-list' $x1 $y1 ($x2 - $x1) ($y2 - $y1)
        [G.U]::SendMsg($ch, $CB_SHOWDROPDOWN, 0, 0) | Out-Null
        Start-Sleep -Milliseconds 300
    } else {
        Write-Host "warn: preset combo not found - skipping 02"
    }

    # 3. Data Collection page (the log-collector half)
    $nav = Find-Nav 'Data Collection'
    if ($nav) {
        Click-El $nav
        Start-Sleep -Milliseconds 600
        Shot-Window '03-data-collection'
    } else {
        Write-Host "warn: 'Data Collection' nav row not found - skipping 03"
    }

    # 4. back to ProcDump, so the last shot matches where step 1 leaves a reader
    $nav = Find-Nav 'ProcDump'
    if ($nav) { Click-El $nav; Start-Sleep -Milliseconds 500 }

    Write-Host "done"
}
finally {
    if ($proc -and -not $proc.HasExited) { $proc.CloseMainWindow() | Out-Null; Start-Sleep 1 }
    if ($proc -and -not $proc.HasExited) { $proc.Kill() }
}

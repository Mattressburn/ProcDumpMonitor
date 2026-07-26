<#
.SYNOPSIS
  Reports the TRUE runtime bitness of a Windows exe, including .NET AnyCPU.

.DESCRIPTION
  ProcDump ships as separate 32-bit and 64-bit binaries and the bitness MUST
  match the target, or the dump is useless -- a 32-bit ProcDump cannot capture
  a 64-bit process at all.

  The PE header's Machine field is NOT sufficient for .NET. A managed PE32
  (Machine=0x014C) with ILONLY set and neither 32BITREQUIRED nor 32BITPREFERRED
  is run as a 64-BIT process by the 64-bit CLR. Reading Machine alone would
  call it 32-bit and pick the wrong ProcDump.

  This mirrors exactly what LogDump's bitness::bitness_from_pe does,
  so it is the field check for "is the tool going to pick the right binary?"

.PARAMETER Path
  One or more exe paths. If omitted, scans every RUNNING process whose name
  starts with "SoftwareHouse." (the C-CURE 9000 process family).

.EXAMPLE
  .\Check-TargetBitness.ps1
  .\Check-TargetBitness.ps1 -Path 'C:\Program Files\Software House\...\Server.exe'

.NOTES
  Compare the RESOLVED column against Task Manager's "Platform"/"32 bit"
  indicator. They must agree. Any disagreement is worth reporting.
#>
param([string[]]$Path)

function Get-TargetBitness([string]$p) {
    $r = [ordered]@{ File = (Split-Path $p -Leaf); Machine = ''; Kind = ''; Resolved = ''; ProcDump = ''; Path = $p }
    if (-not (Test-Path -LiteralPath $p)) { $r.Kind = 'MISSING'; return [pscustomobject]$r }

    $fs = [IO.File]::OpenRead($p); $br = New-Object IO.BinaryReader($fs)
    try {
        $fs.Seek(0x3C, 'Begin') | Out-Null; $off = $br.ReadInt32()
        if ($off -le 0 -or $off + 6 -gt $fs.Length) { $r.Kind = 'NOT-PE'; return [pscustomobject]$r }
        $fs.Seek($off, 'Begin') | Out-Null
        if ($br.ReadUInt32() -ne 0x00004550) { $r.Kind = 'NOT-PE'; return [pscustomobject]$r }

        $machine = $br.ReadUInt16(); $nsec = $br.ReadUInt16()
        $r.Machine = '0x{0:X4}' -f $machine
        $fs.Seek(12, 'Current') | Out-Null
        $optSize = $br.ReadUInt16(); $fs.Seek(2, 'Current') | Out-Null
        $optStart = $fs.Position
        $magic = $br.ReadUInt16()

        # 64-bit machine types are unambiguous.
        if ($machine -eq 0x8664 -or $machine -eq 0xAA64) {
            $r.Kind = 'native'; $r.Resolved = '64-bit'; $r.ProcDump = 'procdump64.exe'
            return [pscustomobject]$r
        }
        if ($machine -ne 0x014C) { $r.Kind = 'unknown-machine'; $r.Resolved = '?'; return [pscustomobject]$r }

        # I386: managed or not? Data directory 14 = CLR runtime header.
        $ddBase = if ($magic -eq 0x20B) { 112 } else { 96 }
        $fs.Seek($optStart + $ddBase + (14 * 8), 'Begin') | Out-Null
        $clrRva = $br.ReadUInt32()
        if ($clrRva -eq 0) {
            $r.Kind = 'native'; $r.Resolved = '32-bit'; $r.ProcDump = 'procdump.exe'
            return [pscustomobject]$r
        }

        # Map the CLR RVA through the section table.
        $secStart = $optStart + $optSize; $fileOff = 0
        for ($i = 0; $i -lt [Math]::Min($nsec, 96); $i++) {
            $fs.Seek($secStart + ($i * 40), 'Begin') | Out-Null
            $null = $br.ReadBytes(8); $null = $br.ReadUInt32()
            $va = $br.ReadUInt32(); $rawSz = $br.ReadUInt32(); $rawPtr = $br.ReadUInt32()
            if ($clrRva -ge $va -and $clrRva -lt ($va + $rawSz)) { $fileOff = $rawPtr + ($clrRva - $va); break }
        }
        if ($fileOff -eq 0 -or $fileOff + 20 -gt $fs.Length) {
            $r.Kind = 'managed(unreadable)'; $r.Resolved = '32-bit'; $r.ProcDump = 'procdump.exe'
            return [pscustomobject]$r
        }

        $fs.Seek($fileOff + 16, 'Begin') | Out-Null      # COR20 Flags is at +16
        $flags = $br.ReadUInt32()
        $req32  = [bool]($flags -band 0x2)               # 32BITREQUIRED
        $pref32 = [bool]($flags -band 0x20000)           # 32BITPREFERRED

        if (-not $req32 -and -not $pref32) {
            $r.Kind = 'managed AnyCPU'; $r.Resolved = '64-bit'; $r.ProcDump = 'procdump64.exe'
        } else {
            $r.Kind = if ($req32) { 'managed x86' } else { 'managed Prefer32' }
            $r.Resolved = '32-bit'; $r.ProcDump = 'procdump.exe'
        }
        [pscustomobject]$r
    } finally { $fs.Close() }
}

if (-not $Path) {
    Write-Host "No -Path given; scanning running SoftwareHouse.* processes..." -ForegroundColor Cyan
    $Path = Get-Process |
        Where-Object { $_.ProcessName -like 'SoftwareHouse.*' } |
        ForEach-Object { try { $_.MainModule.FileName } catch { } } |
        Sort-Object -Unique
    if (-not $Path) {
        Write-Warning "No running SoftwareHouse.* processes found. Run this ON the C-CURE server, elevated."
        return
    }
}

$Path | ForEach-Object { Get-TargetBitness $_ } |
    Format-Table File, Machine, Kind, Resolved, ProcDump -AutoSize

Write-Host ""
Write-Host "Compare RESOLVED against Task Manager's Platform / '32 bit' column." -ForegroundColor Yellow
Write-Host "They must agree. A mismatch means LogDump would pick the wrong binary." -ForegroundColor Yellow

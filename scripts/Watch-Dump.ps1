<#
.SYNOPSIS
    Standalone emergency ProcDump watchdog. Installs itself as a SYSTEM
    scheduled task so monitoring survives closing the PowerShell window,
    user logoff, and reboots.

.DESCRIPTION
    Repeatedly launches ProcDump against a target process or Windows service
    and keeps it running forever, restarting it whenever it exits. This does
    NOT depend on ProcDumpMonitor.exe -- it only needs procdump.exe /
    procdump64.exe -- so it is useful as an immediate safety net on a
    customer machine while the real install is being diagnosed.

    Running the script directly (no switches) just runs the watch loop in
    the current console -- use this to test your parameters first.

    Running with -Install registers a Scheduled Task ("ProcDump Watchdog
    (quick)" by default) that:
      - Starts at boot (AtStartup trigger)
      - Runs as SYSTEM, "whether user is logged on or not"
      - Restarts itself up to 999 times, 1 minute apart, if it ever dies
      - Has no execution time limit
    Once installed, closing the PowerShell window has NO effect on it --
    it runs completely independently, the same way ProcDumpMonitor.exe's
    own scheduled task does.

.PARAMETER TargetName
    Process image name (no .exe) or, with -TargetType Service, the exact
    Windows service name (not the display name).

.PARAMETER DumpDirectory
    Folder dumps are written to. MUST be a local path (e.g. C:\Dumps) or a
    UNC path (\\server\share\Dumps). Do NOT use a mapped drive letter (e.g.
    Z:\Dumps) -- SYSTEM has no drive mappings and will silently fail to
    write there.

.PARAMETER ProcDumpPath
    Full path to procdump.exe / procdump64.exe.

.PARAMETER TargetType
    Process (default) or Service.

.PARAMETER Install
    Register (or re-register) the scheduled task and start it immediately.

.PARAMETER Uninstall
    Remove the scheduled task.

.EXAMPLE
    # Test interactively first (Ctrl+C to stop)
    .\Watch-Dump.ps1 -TargetName MyApp -DumpDirectory C:\Dumps -ProcDumpPath C:\Tools\procdump64.exe

.EXAMPLE
    # Then install so it survives closing the window / reboots
    .\Watch-Dump.ps1 -TargetName MyApp -DumpDirectory C:\Dumps -ProcDumpPath C:\Tools\procdump64.exe -Install

.EXAMPLE
    .\Watch-Dump.ps1 -TargetType Service -TargetName MyWindowsService -DumpDirectory C:\Dumps -ProcDumpPath C:\Tools\procdump64.exe -Install

.EXAMPLE
    .\Watch-Dump.ps1 -Uninstall
#>

[CmdletBinding(DefaultParameterSetName = 'Run')]
param(
    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [Parameter(ParameterSetName = 'Install', Mandatory)]
    [string]$TargetName,

    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [Parameter(ParameterSetName = 'Install', Mandatory)]
    [string]$DumpDirectory,

    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [Parameter(ParameterSetName = 'Install', Mandatory)]
    [string]$ProcDumpPath,

    [Parameter(ParameterSetName = 'Run')]
    [Parameter(ParameterSetName = 'Install')]
    [ValidateSet('Process', 'Service')]
    [string]$TargetType = 'Process',

    [Parameter(ParameterSetName = 'Run')]
    [Parameter(ParameterSetName = 'Install')]
    [int]$RestartDelaySeconds = 5,

    [Parameter(ParameterSetName = 'Install', Mandatory)]
    [switch]$Install,

    [Parameter(ParameterSetName = 'Uninstall', Mandatory)]
    [switch]$Uninstall,

    [Parameter(ParameterSetName = 'Run')]
    [Parameter(ParameterSetName = 'Install')]
    [Parameter(ParameterSetName = 'Uninstall')]
    [string]$TaskName = 'ProcDump Watchdog (quick)'
)

$ErrorActionPreference = 'Stop'

function Register-Watchdog {
    $scriptPath = $PSCommandPath
    $escapedArgs = @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$scriptPath`"",
        '-TargetName', "`"$TargetName`"",
        '-DumpDirectory', "`"$DumpDirectory`"",
        '-ProcDumpPath', "`"$ProcDumpPath`"",
        '-TargetType', $TargetType,
        '-RestartDelaySeconds', $RestartDelaySeconds
    ) -join ' '

    $action    = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $escapedArgs
    $trigger   = New-ScheduledTaskTrigger -AtStartup
    $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
    $settings  = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
        -StartWhenAvailable -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1) `
        -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew

    Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger `
        -Principal $principal -Settings $settings -Force | Out-Null

    Start-ScheduledTask -TaskName $TaskName
    Write-Host "Installed and started scheduled task '$TaskName'."
    Write-Host "It now runs independently of this window and will survive reboots."
    Write-Host "Check status with: Get-ScheduledTask -TaskName '$TaskName' | Get-ScheduledTaskInfo"
}

function Unregister-Watchdog {
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
    Write-Host "Removed scheduled task '$TaskName' (if it existed)."
}

if ($Uninstall) { Unregister-Watchdog; return }
if ($Install) { Register-Watchdog; return }

# ---- Watch loop: this is what actually runs, either interactively or as the scheduled task ----
$logDir = Join-Path $DumpDirectory 'WatchdogLogs'
New-Item -ItemType Directory -Path $DumpDirectory -Force | Out-Null
New-Item -ItemType Directory -Path $logDir -Force | Out-Null
$logFile = Join-Path $logDir 'watchdog.log'

function Write-Log {
    param([string]$Message)
    $line = "[{0:yyyy-MM-dd HH:mm:ss}] $Message" -f (Get-Date)
    Add-Content -Path $logFile -Value $line
    Write-Host $line
}

Write-Log "Watchdog started. Target=$TargetName ($TargetType) DumpDir=$DumpDirectory ProcDump=$ProcDumpPath"

while ($true) {
    try {
        if (-not (Test-Path $ProcDumpPath)) {
            Write-Log "ERROR: ProcDump not found at '$ProcDumpPath'."
            Start-Sleep -Seconds $RestartDelaySeconds
            continue
        }

        # -e     : dump on unhandled (2nd-chance) exception
        # -t     : dump on process termination (catches crashes the app's own
        #          handler swallows before -e would ever see them)
        # -ma    : full memory dump
        # -w     : wait for the process/service to exist if it isn't running yet
        # -accepteula : suppress the EULA prompt (required for unattended runs)
        $pdArgs = @('-accepteula', '-ma', '-e', '-t', '-w')
        if ($TargetType -eq 'Service') {
            $pdArgs += @('-service', $TargetName)
        }
        else {
            $pdArgs += $TargetName
        }
        $pdArgs += $DumpDirectory

        Write-Log "Launching: `"$ProcDumpPath`" $($pdArgs -join ' ')"

        $proc = Start-Process -FilePath $ProcDumpPath -ArgumentList $pdArgs -NoNewWindow -PassThru `
            -RedirectStandardOutput (Join-Path $logDir 'procdump.out.log') `
            -RedirectStandardError  (Join-Path $logDir 'procdump.err.log')
        $proc.WaitForExit()

        Write-Log "ProcDump exited with code $($proc.ExitCode)."
    }
    catch {
        Write-Log "Loop error: $($_.Exception.Message)"
    }

    Start-Sleep -Seconds $RestartDelaySeconds
}

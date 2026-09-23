param(
    [Parameter(Mandatory = $true)]
    [int]$StartAfter,
    [int]$Seconds = 30,
    [int]$Threads = 24,
    [int]$CompletePrefixBase = 113,
    [string]$OutputRoot = "outputs/automatic",
    [int]$MaxCrashRestarts = 3
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $projectRoot "target/release/apa-exact-search.exe"
$candidates = Join-Path $projectRoot "progress/candidates.txt"
$outputBase = Join-Path $projectRoot $OutputRoot
$statePath = Join-Path $outputBase "state.json"
$attentionPath = Join-Path $outputBase "attention.json"
$logPath = Join-Path $outputBase "supervisor.log"

New-Item -ItemType Directory -Force -Path $outputBase | Out-Null

function Write-JsonAtomic {
    param([string]$Path, [object]$Value)
    $temporary = "$Path.tmp"
    $Value | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $temporary -Encoding utf8
    Move-Item -LiteralPath $temporary -Destination $Path -Force
}

function Save-State {
    param([string]$Status, [int]$LastNo, [Nullable[int]]$AttentionMaximum, [string]$RunDirectory)
    Write-JsonAtomic -Path $statePath -Value ([ordered]@{
        status = $Status
        last_no = $LastNo
        attention_maximum = $AttentionMaximum
        run_directory = $RunDirectory
        updated_at = (Get-Date).ToString("o")
        seconds_per_candidate = $Seconds
        threads = $Threads
        complete_prefix_base = $CompletePrefixBase
    })
}

if (Test-Path -LiteralPath $statePath) {
    $saved = Get-Content -Raw -LiteralPath $statePath | ConvertFrom-Json
    $savedLastNo = [int]$saved.last_no
    if ($saved.status -eq "RUNNING") {
        $activeSolver = Get-Process apa-exact-search -ErrorAction SilentlyContinue
        if ($activeSolver) {
            throw "The saved supervisor state is RUNNING and a solver process exists."
        }
        Add-Content -LiteralPath $logPath -Value "$(Get-Date -Format o) recovering stale RUNNING state"
    }
    if ($StartAfter -gt $savedLastNo) {
        throw "StartAfter $StartAfter would skip the saved contiguous NO boundary $savedLastNo. Resolve and record the attention case before advancing."
    }
    $StartAfter = $savedLastNo
}

$other = Get-Process apa-exact-search -ErrorAction SilentlyContinue
if ($other) {
    throw "apa-exact-search is already running (PID $($other.Id -join ', '))."
}
if (-not (Test-Path -LiteralPath $executable)) {
    throw "Release executable not found: $executable"
}
if (-not (Test-Path -LiteralPath $candidates)) {
    throw "Candidate source not found: $candidates"
}

$crashes = 0
while ($true) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
    $runDirectory = Join-Path $outputBase "run-$stamp"
    New-Item -ItemType Directory -Path $runDirectory | Out-Null
    Save-State -Status "RUNNING" -LastNo $StartAfter -AttentionMaximum $null -RunDirectory $runDirectory
    Add-Content -LiteralPath $logPath -Value "$(Get-Date -Format o) start_after=$StartAfter run=$runDirectory"

    $arguments = @(
        "campaign",
        "--candidates", $candidates,
        "--start-after", $StartAfter,
        "--output", $runDirectory,
        "--seconds", $Seconds,
        "--threads", $Threads
        "--complete-prefix-base", $CompletePrefixBase
    )
    & $executable @arguments 2>&1 | Tee-Object -FilePath (Join-Path $runDirectory "campaign.log") -Append
    $exitCode = $LASTEXITCODE
    $campaignPath = Join-Path $runDirectory "campaign.json"

    if (-not (Test-Path -LiteralPath $campaignPath)) {
        $crashes += 1
        Add-Content -LiteralPath $logPath -Value "$(Get-Date -Format o) missing campaign.json exit=$exitCode retry=$crashes"
        if ($crashes -le $MaxCrashRestarts) {
            continue
        }
        $attention = [ordered]@{
            status = "OPERATIONAL_ERROR"
            maximum = $null
            reason = "campaign.json was not produced after repeated restarts"
            exit_code = $exitCode
            run_directory = $runDirectory
            updated_at = (Get-Date).ToString("o")
        }
        Write-JsonAtomic -Path $attentionPath -Value $attention
        Save-State -Status "OPERATIONAL_ERROR" -LastNo $StartAfter -AttentionMaximum $null -RunDirectory $runDirectory
        exit 12
    }

    # Deliberately enumerate JSON arrays into individual report rows.
    $parsedRows = Get-Content -Raw -LiteralPath $campaignPath | ConvertFrom-Json
    $rows = @($parsedRows | ForEach-Object { $_ })
    $validatedLastNo = $StartAfter
    $attentionRow = $null
    foreach ($row in $rows) {
        $rowMaximum = [Convert]::ToInt32($row.maximum)
        if ($rowMaximum -le $validatedLastNo) {
            throw "Campaign results are not strictly increasing after $validatedLastNo."
        }
        if ($null -ne $attentionRow) {
            throw "Campaign contains a result after the first non-NO row."
        }
        if ([string]$row.status -eq "NO") {
            $validatedLastNo = $rowMaximum
        } else {
            $attentionRow = $row
        }
    }
    $StartAfter = $validatedLastNo
    if ($attentionRow) {
        $attentionMaximum = [Convert]::ToInt32($attentionRow.maximum)
        $attention = [ordered]@{
            status = [string]$attentionRow.status
            maximum = $attentionMaximum
            result = (Join-Path $runDirectory "$($attentionRow.maximum).json")
            run_directory = $runDirectory
            updated_at = (Get-Date).ToString("o")
        }
        Write-JsonAtomic -Path $attentionPath -Value $attention
        Save-State -Status ([string]$attentionRow.status) -LastNo $StartAfter -AttentionMaximum $attentionMaximum -RunDirectory $runDirectory
        exit 10
    }
    if ($exitCode -ne 0) {
        $crashes += 1
        if ($crashes -le $MaxCrashRestarts) {
            continue
        }
        $attention = [ordered]@{
            status = "OPERATIONAL_ERROR"
            maximum = $null
            reason = "campaign exited repeatedly with code $exitCode"
            run_directory = $runDirectory
            updated_at = (Get-Date).ToString("o")
        }
        Write-JsonAtomic -Path $attentionPath -Value $attention
        Save-State -Status "OPERATIONAL_ERROR" -LastNo $StartAfter -AttentionMaximum $null -RunDirectory $runDirectory
        exit 12
    }

    Remove-Item -LiteralPath $attentionPath -Force -ErrorAction SilentlyContinue
    Save-State -Status "COMPLETE" -LastNo $StartAfter -AttentionMaximum $null -RunDirectory $runDirectory
    exit 0
}

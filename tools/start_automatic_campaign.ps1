param(
    [Parameter(Mandatory = $true)]
    [int]$StartAfter,
    [int]$Seconds = 30,
    [int]$Threads = 24,
    [int]$CompletePrefixBase = 113,
    [string]$OutputRoot = "outputs/automatic"
)

$ErrorActionPreference = "Stop"
$worker = Join-Path $PSScriptRoot "run_automatic_campaign.ps1"
$projectRoot = Split-Path -Parent $PSScriptRoot
$outputBase = Join-Path $projectRoot $OutputRoot
New-Item -ItemType Directory -Force -Path $outputBase | Out-Null
$statePath = Join-Path $outputBase "state.json"
if (Test-Path -LiteralPath $statePath) {
    $saved = Get-Content -Raw -LiteralPath $statePath | ConvertFrom-Json
    $savedLastNo = [int]$saved.last_no
    if ($StartAfter -gt $savedLastNo) {
        throw "StartAfter $StartAfter would skip the saved contiguous NO boundary $savedLastNo. Resolve and record the attention case before advancing."
    }
}
$stdout = Join-Path $outputBase "launcher.stdout.log"
$stderr = Join-Path $outputBase "launcher.stderr.log"
$pwsh = Join-Path $env:LOCALAPPDATA "Microsoft\WindowsApps\pwsh.exe"
if (-not (Test-Path -LiteralPath $pwsh)) {
    throw "PowerShell 7 app execution alias not found: $pwsh"
}

$arguments = @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $worker,
    "-StartAfter", $StartAfter,
    "-Seconds", $Seconds,
    "-Threads", $Threads,
    "-CompletePrefixBase", $CompletePrefixBase,
    "-OutputRoot", $OutputRoot
)
$process = Start-Process -FilePath $pwsh -ArgumentList $arguments `
    -WorkingDirectory $projectRoot -WindowStyle Hidden -RedirectStandardOutput $stdout `
    -RedirectStandardError $stderr -PassThru

[ordered]@{
    supervisor_pid = $process.Id
    state = $statePath
    attention = (Join-Path $outputBase "attention.json")
    stdout = $stdout
    stderr = $stderr
} | ConvertTo-Json

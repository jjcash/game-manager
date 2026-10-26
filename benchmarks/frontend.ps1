# SPDX-License-Identifier: GPL-3.0-or-later
param(
    [Parameter(Mandatory = $true)][string]$DatabaseDirectory,
    [Parameter(Mandatory = $true)][string]$ResultsDirectory,
    [int[]]$Sizes = @(500, 12320, 15000, 25000),
    [int]$Runs = 2,
    [switch]$QueryScenarios,
    [string]$ArtworkDirectory,
    [switch]$ArtworkScenarios,
    [switch]$ArtworkIpcScenarios,
    [switch]$ProfileArtwork,
    [switch]$NoMemorySeries,
    [string]$BenchmarkExecutable,
    [switch]$NormalSmoke,
    [ValidateRange(1,50)][int]$ArtworkCycles = 3
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $repo 'apps/desktop/src-tauri/target/release/game-manager.exe'
if ($BenchmarkExecutable) { $executable = (Resolve-Path -LiteralPath $BenchmarkExecutable).Path }
if (!(Test-Path -LiteralPath $executable)) { throw 'Build the native app with --features benchmarks first.' }
if (Get-Process game-manager -ErrorAction SilentlyContinue) { throw 'Close existing Game Manager windows first.' }
if (Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue) { throw 'Port 9223 is already in use.' }
$databaseRoot = (Resolve-Path -LiteralPath $DatabaseDirectory).Path
$resultsRoot = [IO.Path]::GetFullPath($ResultsDirectory)
if (Test-Path -LiteralPath $resultsRoot) { throw 'Choose a new results directory.' }
New-Item -ItemType Directory -Path $resultsRoot | Out-Null
$savedDatabase = $env:GAME_MANAGER_BENCHMARK_DB
$savedArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$savedProfile = $env:WEBVIEW2_USER_DATA_FOLDER
$savedDataRoot = $env:GAME_MANAGER_DATA_DIR
$savedArtwork = $env:GAME_MANAGER_BENCHMARK_ARTWORK
function Get-AppMemory([int]$RootId) {
    $processes = Get-CimInstance Win32_Process
    $ids = @($RootId)
    do {
        $children = @($processes | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids } | ForEach-Object { [int]$_.ProcessId })
        $ids += $children
    } while ($children.Count -gt 0)
    $live = @(Get-Process -Id $ids -ErrorAction SilentlyContinue)
    return @{ processes = $live.Count; privateBytes = ($live | Measure-Object PrivateMemorySize64 -Sum).Sum; workingSetBytes = ($live | Measure-Object WorkingSet64 -Sum).Sum; nativePrivateBytes = (Get-Process -Id $RootId).PrivateMemorySize64 }
}
try {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
    $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $resultsRoot 'webview-profile'
    # Even an accidentally ordinary build must not open the user's normal DB.
    $env:GAME_MANAGER_DATA_DIR = Join-Path $resultsRoot 'application-fallback'
    foreach ($size in $Sizes) {
        if ($size -notin @(500, 2000, 5000, 12320, 15000, 25000)) { throw "Unsupported size: $size" }
        $database = Join-Path $databaseRoot "synthetic-v1-seed-42-$size.sqlite3"
        if (!(Test-Path -LiteralPath $database)) { throw "Missing dataset: $database" }
        $beforeHash = (Get-FileHash -LiteralPath $database).Hash
        for ($run = 1; $run -le $Runs; $run++) {
            $env:GAME_MANAGER_BENCHMARK_DB = $database
            if ($ArtworkDirectory) {
                $env:GAME_MANAGER_BENCHMARK_ARTWORK = (Resolve-Path -LiteralPath (Join-Path $ArtworkDirectory "artwork-$size")).Path
            } else { $env:GAME_MANAGER_BENCHMARK_ARTWORK = $null }
            $appProcess = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $resultsRoot "$size-$run.stderr.txt")
            try {
                $driver = if ($NormalSmoke) { 'frontend-normal.mjs' } elseif ($ArtworkIpcScenarios) { 'frontend-artwork-ipc.mjs' } elseif ($ArtworkScenarios) { 'frontend-artwork.mjs' } elseif ($QueryScenarios) { 'frontend-query.mjs' } else { 'frontend.mjs' }
                $memorySamples = [System.Collections.Generic.List[object]]::new()
                $clock = [Diagnostics.Stopwatch]::StartNew()
                $drivers = @(@{name = $driver; suffix = ''})
                if ($ArtworkScenarios) { $drivers += @{name = 'frontend-query.mjs'; suffix = '.query'} }
                foreach ($scenario in $drivers) {
                    $reportPath = Join-Path $resultsRoot "$size-$run$($scenario.suffix).json"
                    $nodeArguments = @(('"' + (Join-Path $PSScriptRoot $scenario.name) + '"'), 'http://127.0.0.1:9223', ('"' + $reportPath + '"'))
                    if ($ProfileArtwork -and $scenario.name -eq 'frontend-artwork.mjs') { $nodeArguments += '--profile' }
                    if ($scenario.name -eq 'frontend-artwork.mjs') { $nodeArguments += "--cycles=$ArtworkCycles" }
                    $driverProcess = Start-Process -FilePath (Get-Command node).Source -ArgumentList $nodeArguments -WindowStyle Hidden -PassThru -RedirectStandardOutput "$reportPath.stdout.txt" -RedirectStandardError "$reportPath.stderr.txt"
                    # Retain the process handle while sampling; refreshing an exited
                    # Process can discard its cached Windows exit status.
                    $null = $driverProcess.Handle
                    do {
                        $exited = $driverProcess.WaitForExit(2000)
                        if (!$NoMemorySeries) { $memorySamples.Add(@{elapsedMs = $clock.ElapsedMilliseconds; utc = [DateTime]::UtcNow.ToString('o'); scenario = $scenario.name; memory = (Get-AppMemory $appProcess.Id)}) }
                    } while (!$exited)
                    $driverProcess.WaitForExit()
                    if ($driverProcess.ExitCode -ne 0) { throw "Frontend scenario failed for $size run $run; see $reportPath.stderr.txt" }
                }
                $memorySamples | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $resultsRoot "$size-$run.memory-series.json")
                Get-AppMemory $appProcess.Id | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $resultsRoot "$size-$run.memory.json")
            } finally {
                $appProcess.Refresh()
                if (!$appProcess.HasExited) { $null = $appProcess.CloseMainWindow(); $appProcess.WaitForExit(10000) | Out-Null }
                if (!$appProcess.HasExited) { throw 'App did not close normally; close it before continuing.' }
            }
        }
        if ((Get-FileHash -LiteralPath $database).Hash -ne $beforeHash) { throw "Dataset changed: $database" }
    }
} finally {
    $env:GAME_MANAGER_BENCHMARK_DB = $savedDatabase
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $savedArguments
    $env:WEBVIEW2_USER_DATA_FOLDER = $savedProfile
    $env:GAME_MANAGER_DATA_DIR = $savedDataRoot
    $env:GAME_MANAGER_BENCHMARK_ARTWORK = $savedArtwork
}

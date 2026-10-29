# SPDX-License-Identifier: GPL-3.0-or-later
param(
 [Parameter(Mandatory=$true)][string]$ResultsDirectory,
 [Parameter(Mandatory=$true)][string]$BaselineExecutable,
 [string]$DatabaseDirectory,
 [string]$Executable,
 [ValidateRange(1,100)][int]$BaselineSchema=1,
 [ValidateRange(1,100)][int]$CandidateSchema=2
)
$ErrorActionPreference='Stop'
if(!$DatabaseDirectory){$DatabaseDirectory=Join-Path $PSScriptRoot 'data/m0.2-2026-10-25-01'}
if(!$Executable){$Executable=Join-Path $PSScriptRoot '../apps/desktop/src-tauri/target/release/game-manager.exe'}
$Executable=(Resolve-Path -LiteralPath $Executable).Path
$BaselineExecutable=(Resolve-Path -LiteralPath $BaselineExecutable).Path
if(Get-Process game-manager -ErrorAction SilentlyContinue){throw 'Close Game Manager first'}
if(Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue){throw 'Port 9223 occupied'}
$root=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $root){throw 'Use a new results root'}
New-Item -ItemType Directory -Path $root | Out-Null
$names=@('GAME_MANAGER_DATA_DIR','GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_SYNTHETIC_ROOT','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','WEBVIEW2_USER_DATA_FOLDER')
$saved=@{};foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
try {
 $env:GAME_MANAGER_BENCHMARK_DB=$null;$env:GAME_MANAGER_SYNTHETIC_ROOT=$null
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
 foreach($size in @(15000,25000)) {
  $source=Join-Path $DatabaseDirectory "synthetic-v1-seed-42-$size.sqlite3";$hash=(Get-FileHash -LiteralPath $source).Hash
  for($pass=0;$pass -lt 2;$pass++) {
   $order=if($pass -eq 0){@(1,2)}else{@(2,1)}
   foreach($version in $order) {
    $run=Join-Path $root "$size-$pass-v$version";New-Item -ItemType Directory -Path "$run/application/database" | Out-Null
    $copy=Join-Path $run 'application/database/library.sqlite3'
    # SQLite backup, then change purpose only on the newly owned destination.
    & python -B "$PSScriptRoot/prepare-manual-fixture.py" $source $copy
    if($LASTEXITCODE -ne 0){throw 'Fixture copy failed'}
    $env:GAME_MANAGER_DATA_DIR=Join-Path $run 'application';$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $run 'webview-profile'
    $binary=if($version -eq 1){$BaselineExecutable}else{$Executable}
    $expectedSchema=if($version -eq 1){$BaselineSchema}else{$CandidateSchema}
    $app=Start-Process -FilePath ([IO.Path]::GetFullPath($binary)) -WindowStyle Hidden -PassThru -RedirectStandardError "$run/stderr.txt"
    try {
     & node "$PSScriptRoot/frontend-manual-query.mjs" 'http://127.0.0.1:9223' "$run/query.json" $expectedSchema
     if($LASTEXITCODE -ne 0){throw 'Application query regression failed'}
    } finally {$app.Refresh();if(!$app.HasExited){$null=$app.CloseMainWindow();$null=$app.WaitForExit(10000)};if(!$app.HasExited){throw 'App did not close normally'}}
   }
  }
  if((Get-FileHash -LiteralPath $source).Hash -ne $hash){throw 'Source fixture changed'}
 }
 & python -B "$PSScriptRoot/summarize-manual-query.py" $root | Set-Content -Encoding utf8 -LiteralPath "$root/summary.json"
 if($LASTEXITCODE -ne 0){throw 'Cross-version query equivalence failed'}
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}

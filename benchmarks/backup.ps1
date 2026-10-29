# SPDX-License-Identifier: GPL-3.0-or-later
# Interactive native file-pickers: follow frontend-backup.mjs console instructions.
param([Parameter(Mandatory=$true)][string]$ResultsDirectory,[string]$Executable)
$ErrorActionPreference='Stop'
if(!$Executable){$Executable=Join-Path $PSScriptRoot '../apps/desktop/src-tauri/target/release/game-manager.exe'}
$binary=(Resolve-Path -LiteralPath $Executable).Path
if(Get-Process game-manager -ErrorAction SilentlyContinue){throw 'Close Game Manager first'}
if(Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue){throw 'Port 9223 occupied'}
$root=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $root){throw 'Use a NEW disposable output root'}
New-Item -ItemType Directory -Path $root | Out-Null
& python "$PSScriptRoot/prepare-backup-validation-fixture.py" "$PSScriptRoot/data/m0.2-2026-10-25-01/synthetic-v1-seed-42-500.sqlite3" "$root/benchmark-purpose.sqlite3"
if($LASTEXITCODE -ne 0){throw 'Wrong-purpose fixture preparation failed'}
$names=@('GAME_MANAGER_DATA_DIR','GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_SYNTHETIC_ROOT','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','WEBVIEW2_USER_DATA_FOLDER')
$saved=@{}
foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
function Close-Smoke {
  $apps=@(Get-Process game-manager -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $binary})
  if($apps.Count -gt 1){throw 'Unexpected duplicate smoke processes; inspect manually'}
  foreach($app in $apps){$null=$app.CloseMainWindow();$null=$app.WaitForExit(10000);if(!$app.HasExited){throw 'Smoke process did not close normally'}}
}
try {
  $env:GAME_MANAGER_DATA_DIR=Join-Path $root 'application'
  $env:GAME_MANAGER_BENCHMARK_DB='invalid-config-ignored'
  $env:GAME_MANAGER_SYNTHETIC_ROOT='invalid-config-ignored'
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
  $env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $root 'webview-profile'
  foreach($phase in @('flow','verify')){
    $app=Start-Process -FilePath $binary -WindowStyle Hidden -PassThru -RedirectStandardError "$root/$phase.stderr.txt"
    try {
      & node "$PSScriptRoot/frontend-backup.mjs" 'http://127.0.0.1:9223' "$root/$phase.json" $phase "$root/identities.json"
      if($LASTEXITCODE -ne 0){throw "Native backup $phase failed"}
      if(Test-Path -LiteralPath "$root/application/database/library.sqlite3.restore-pending"){throw 'Completed restore retained active recovery authority'}
      if(!(Test-Path -LiteralPath "$root/application/database/library.sqlite3.restore-last")){throw 'Completed restore has no historical status reference'}
    }finally{Close-Smoke}
  }
}finally{foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}

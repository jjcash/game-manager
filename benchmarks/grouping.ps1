# SPDX-License-Identifier: GPL-3.0-or-later
param([Parameter(Mandatory=$true)][string]$ResultsDirectory,[string]$Executable)
$ErrorActionPreference='Stop'
if(!$Executable){$Executable=Join-Path $PSScriptRoot '../apps/desktop/src-tauri/target/release/game-manager.exe'}
$binary=(Resolve-Path -LiteralPath $Executable).Path
if(Get-Process game-manager -ErrorAction SilentlyContinue){throw 'Close Game Manager first'}
if(Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue){throw 'Port 9223 occupied'}
$root=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $root){throw 'Use a new disposable output root'}
New-Item -ItemType Directory -Path $root | Out-Null
$names=@('GAME_MANAGER_DATA_DIR','GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_SYNTHETIC_ROOT','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','WEBVIEW2_USER_DATA_FOLDER')
$saved=@{}
foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
try {
  $env:GAME_MANAGER_DATA_DIR=Join-Path $root 'application'
  $env:GAME_MANAGER_BENCHMARK_DB='invalid-benchmark-config-ignored'
  $env:GAME_MANAGER_SYNTHETIC_ROOT='invalid-synthetic-config-ignored'
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
  $env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $root 'webview-profile'
  foreach($phase in @('organize','restart')) {
    $app=Start-Process -FilePath $binary -WindowStyle Hidden -PassThru -RedirectStandardError "$root/$phase.stderr.txt"
    try {
      & node "$PSScriptRoot/frontend-grouping.mjs" 'http://127.0.0.1:9223' "$root/$phase.json" $phase "$root/identities.json"
      if($LASTEXITCODE -ne 0){throw "Grouping $phase failed"}
    } finally {
      $app.Refresh()
      if(!$app.HasExited){$null=$app.CloseMainWindow();$null=$app.WaitForExit(10000)}
      if(!$app.HasExited){throw 'App did not close normally'}
    }
  }
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}

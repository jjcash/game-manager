# SPDX-License-Identifier: GPL-3.0-or-later
param([Parameter(Mandatory=$true)][string]$ResultsDirectory,[string]$Executable)
$ErrorActionPreference='Stop'
if(!$Executable){$Executable=Join-Path $PSScriptRoot '../apps/desktop/src-tauri/target/release/game-manager.exe'}
$binary=(Resolve-Path -LiteralPath $Executable).Path
if(Get-Process game-manager -ErrorAction SilentlyContinue){throw 'Close Game Manager first'}
if(Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue){throw 'Port 9223 occupied'}
$root=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $root){throw 'Use a new disposable root'}
New-Item -ItemType Directory -Path $root | Out-Null
. "$PSScriptRoot/process-memory.ps1"
$names=@('GAME_MANAGER_DATA_DIR','GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_SYNTHETIC_ROOT','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','WEBVIEW2_USER_DATA_FOLDER')
$saved=@{};foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
try {
 $env:GAME_MANAGER_BENCHMARK_DB=$null;$env:GAME_MANAGER_SYNTHETIC_ROOT=$null
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
 foreach($size in @(15000,25000)){
  $run=Join-Path $root $size;New-Item -ItemType Directory -Path "$run/application/database" | Out-Null
  $source=Join-Path $PSScriptRoot "data/m0.2-2026-10-25-01/synthetic-v1-seed-42-$size.sqlite3"
  $hash=(Get-FileHash -LiteralPath $source).Hash
  & python -B "$PSScriptRoot/prepare-manual-fixture.py" $source "$run/application/database/library.sqlite3"
  if($LASTEXITCODE -ne 0){throw 'Application fixture preparation failed'}
  $env:GAME_MANAGER_DATA_DIR=Join-Path $run 'application';$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $run 'webview-profile'
  foreach($phase in @('initialize','activity')){
   if($phase -eq 'activity'){
    & python -B "$PSScriptRoot/prepare-artwork-activity.py" "$run/application/database/library.sqlite3"
    if($LASTEXITCODE -ne 0){throw 'Artwork fixture attachment failed'}
   }
   $app=Start-Process -FilePath $binary -WindowStyle Hidden -PassThru -RedirectStandardError "$run/$phase.stderr.txt"
   try {
    if($phase -eq 'activity'){
     $driver=Start-Process -FilePath (Get-Command node).Source -ArgumentList @("`"$PSScriptRoot/frontend-artwork-activity.mjs`"",'http://127.0.0.1:9223',"`"$run/$phase.json`"",$phase) -WindowStyle Hidden -PassThru -RedirectStandardOutput "$run/driver.txt" -RedirectStandardError "$run/driver-error.txt"
     # Retain the process handle before polling so Windows PowerShell preserves
     # the exit code after the short-lived driver exits.
     $null=$driver.Handle
     $series=@();while(!$driver.WaitForExit(500)){$series+=@(Get-AppMemory $app.Id);$driver.Refresh()}
     $series | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 -LiteralPath "$run/memory.json"
     $driver.WaitForExit();$driver.Refresh()
     if($driver.ExitCode -ne 0){throw "Artwork activity driver exit code '$($driver.ExitCode)'; inspect driver logs"}
    }else{
     & node "$PSScriptRoot/frontend-artwork-activity.mjs" 'http://127.0.0.1:9223' "$run/$phase.json" $phase
     if($LASTEXITCODE -ne 0){throw 'Initialization failed'}
    }
   }finally{$app.Refresh();if(!$app.HasExited){$null=$app.CloseMainWindow();$null=$app.WaitForExit(10000)};if(!$app.HasExited){throw 'App did not close normally'}}
  }
  if((Get-FileHash -LiteralPath $source).Hash -ne $hash){throw 'Frozen source changed'}
 }
}finally{foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}

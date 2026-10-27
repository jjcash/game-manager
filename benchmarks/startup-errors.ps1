# SPDX-License-Identifier: GPL-3.0-or-later
param([Parameter(Mandatory=$true)][string]$BenchmarkExecutable,[Parameter(Mandatory=$true)][string]$ResultsDirectory,[Parameter(Mandatory=$true)][string]$DatabaseDirectory)
$ErrorActionPreference='Stop'
if(Get-Process game-manager -ErrorAction SilentlyContinue){throw 'Close Game Manager first'}
if(Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue){throw 'Debug port occupied'}
$exe=(Resolve-Path -LiteralPath $BenchmarkExecutable).Path
$root=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $root){throw 'Choose a new results directory'}
New-Item -ItemType Directory -Path $root | Out-Null
$names=@('GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_DATA_DIR','GAME_MANAGER_STARTUP_TIMING','GAME_MANAGER_STARTUP_PROFILE_WINDOW','GAME_MANAGER_SYNTHETIC_ROOT','GAME_MANAGER_BENCHMARK_ARTWORK','GAME_MANAGER_ARTWORK_WORKERS','GAME_MANAGER_BACKGROUND_POLICY','WEBVIEW2_USER_DATA_FOLDER','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS')
$saved=@{}
foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process');[Environment]::SetEnvironmentVariable($name,$null,'Process')}
try{
 $env:GAME_MANAGER_STARTUP_TIMING='1'
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
 foreach($case in @('empty','missing','directory','corrupt','locked','blocked-data-root')){
  $caseRoot=Join-Path $root $case
  New-Item -ItemType Directory -Path $caseRoot | Out-Null
  $env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $caseRoot 'profile'
  $env:GAME_MANAGER_DATA_DIR=Join-Path $caseRoot 'app'
  $env:GAME_MANAGER_BENCHMARK_DB=if($case -in @('empty','blocked-data-root')){$null}else{Join-Path $caseRoot 'fixture.sqlite3'}
  $before=$null
  $lockedFile=$null
  if($case -eq 'locked'){
   Copy-Item -LiteralPath (Join-Path $DatabaseDirectory 'synthetic-v1-seed-42-500.sqlite3') -Destination $env:GAME_MANAGER_BENCHMARK_DB
   $before=(Get-FileHash -LiteralPath $env:GAME_MANAGER_BENCHMARK_DB).Hash
   $lockedFile=[IO.File]::Open($env:GAME_MANAGER_BENCHMARK_DB,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None)
  }
  if($case -eq 'directory'){New-Item -ItemType Directory -Path $env:GAME_MANAGER_BENCHMARK_DB | Out-Null}
  if($case -eq 'corrupt'){[IO.File]::WriteAllText($env:GAME_MANAGER_BENCHMARK_DB,'deliberately invalid disposable database');$before=(Get-FileHash -LiteralPath $env:GAME_MANAGER_BENCHMARK_DB).Hash}
  if($case -eq 'blocked-data-root'){[IO.File]::WriteAllText($env:GAME_MANAGER_DATA_DIR,'owned blocking file')}
  $appProcess=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $caseRoot 'app.stderr.txt')
  $null=$appProcess.Handle
  try{
   & node "$PSScriptRoot/frontend-startup-errors.mjs" http://127.0.0.1:9223 (Join-Path $caseRoot 'result.json') $case
   if($LASTEXITCODE -ne 0){throw "Failed $case"}
  }finally{
   $appProcess.Refresh()
   if(!$appProcess.HasExited){$null=$appProcess.CloseMainWindow()}
   if(!$appProcess.WaitForExit(10000)){throw 'Failed safe shutdown'}
   if($lockedFile){$lockedFile.Dispose()}
   if($appProcess.ExitCode -ne 0){throw 'Failed process exit'}
  }
  if($case -eq 'missing' -and (Test-Path -LiteralPath $env:GAME_MANAGER_BENCHMARK_DB)){throw 'Missing database was created'}
  if($before -and (Get-FileHash -LiteralPath $env:GAME_MANAGER_BENCHMARK_DB).Hash -ne $before){throw 'Invalid file changed'}
  Write-Output "$case passed"
 }
}finally{foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}

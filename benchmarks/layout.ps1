# SPDX-License-Identifier: GPL-3.0-or-later
param(
    [Parameter(Mandatory=$true)][string]$ResultsDirectory,
    [Parameter(Mandatory=$true)][string[]]$Executables,
    [string]$DatabaseDirectory = "$PSScriptRoot/data/m0.2-2026-10-25-01",
    [int[]]$Sizes = @(15000,25000),
    [ValidateRange(1,30)][int]$Repetitions = 6,
    [string]$Variants = 'baseline',
    [switch]$ClosedDiagnostics,
    [switch]$NoArtwork,
    [switch]$Untraced
)
$ErrorActionPreference='Stop'
if(Get-Process game-manager -ErrorAction SilentlyContinue){throw 'Close Game Manager first.'}
if(Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue){throw 'Port 9223 occupied.'}
$results=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $results){throw 'Use a new output directory.'}
New-Item -ItemType Directory -Path $results | Out-Null
$names=@('GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_BENCHMARK_ARTWORK','GAME_MANAGER_SYNTHETIC_ROOT','GAME_MANAGER_ARTWORK_WORKERS','GAME_MANAGER_BACKGROUND_POLICY','GAME_MANAGER_DATA_DIR','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','WEBVIEW2_USER_DATA_FOLDER')
$saved=@{}
foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
. "$PSScriptRoot/process-memory.ps1"
try {
    $env:GAME_MANAGER_SYNTHETIC_ROOT=$null
    $env:GAME_MANAGER_ARTWORK_WORKERS=$null
    $env:GAME_MANAGER_BACKGROUND_POLICY=$null
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
    foreach($size in $Sizes){
        if($size -notin @(15000,25000)){throw 'Use release/stress size.'}
        $db=(Resolve-Path -LiteralPath (Join-Path $DatabaseDirectory "synthetic-v1-seed-42-$size.sqlite3")).Path
        $before=(Get-FileHash -LiteralPath $db).Hash
        for($artifact=0;$artifact -lt $Executables.Count;$artifact++){
            $exe=(Resolve-Path -LiteralPath $Executables[$artifact]).Path
            $prefix=Join-Path $results "$size-$artifact"
            $env:GAME_MANAGER_BENCHMARK_DB=$db
            $env:GAME_MANAGER_BENCHMARK_ARTWORK=if($NoArtwork){$null}else{(Resolve-Path -LiteralPath "$PSScriptRoot/data/artwork-$size").Path}
            $env:GAME_MANAGER_DATA_DIR="$prefix-fallback"
            $env:WEBVIEW2_USER_DATA_FOLDER="$prefix-profile"
            @{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;database=$db;databaseSha256=$before;artwork=$env:GAME_MANAGER_BENCHMARK_ARTWORK;diagnosticsOpen=(!$ClosedDiagnostics);traced=(!$Untraced)} | ConvertTo-Json | Set-Content -LiteralPath "$prefix.environment.json" -Encoding utf8
            $appProcess=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru -RedirectStandardError "$prefix.app.stderr.txt"
            $null=$appProcess.Handle
            try {
                $arguments=@("$PSScriptRoot/frontend-layout.mjs",'http://127.0.0.1:9223',"$prefix.json","--repetitions=$Repetitions","--variants=$Variants")
                if($ClosedDiagnostics){$arguments+='--closed-diagnostics'}
                if($Untraced){$arguments+='--untraced'}
                & node @arguments | Tee-Object -FilePath "$prefix.stdout.txt"
                if($LASTEXITCODE -ne 0){throw "Layout driver failed: $prefix"}
                Get-AppMemory $appProcess.Id | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$prefix.memory.json"
            } finally {
                if(!$appProcess.HasExited){$null=$appProcess.CloseMainWindow()}
                if(!$appProcess.WaitForExit(10000)){throw 'Native shutdown did not finish.'}
                if($appProcess.ExitCode -ne 0){throw 'Native shutdown failed.'}
            }
        }
        if((Get-FileHash -LiteralPath $db).Hash -ne $before){throw 'Source database changed.'}
    }
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}

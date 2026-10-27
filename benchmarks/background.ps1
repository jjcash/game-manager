# SPDX-License-Identifier: GPL-3.0-or-later
param(
    [Parameter(Mandatory=$true)][string]$DatabaseDirectory,
    [Parameter(Mandatory=$true)][string]$ArtworkDirectory,
    [Parameter(Mandatory=$true)][string]$ResultsDirectory,
    [int[]]$Sizes=@(500,12320,15000,25000),
    [string[]]$Cases=@('idle','cpu','hash','write','art1','art2','mixed0','mixed1','mixed2','priority'),
    [ValidateRange(5,100)][int]$Iterations=20,
    [ValidateRange(1,20)][int]$Cycles=1,
    [switch]$SampleMemory,
    [switch]$Trace,
    [switch]$TraceSort,
    [switch]$ClosedDiagnostics,
    [switch]$ShutdownJob,
    [switch]$RestartCheck
)
$ErrorActionPreference='Stop'
if($Trace -and $TraceSort){throw 'Select one tracing mode.'}
$repo=Split-Path $PSScriptRoot -Parent
$exe=Join-Path $repo 'apps/desktop/src-tauri/target/release/game-manager.exe'
$generator=Join-Path $repo 'apps/desktop/src-tauri/target/release/job-fixtures.exe'
if (Get-Process game-manager -ErrorAction SilentlyContinue) {throw 'Close Game Manager first.'}
if (Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue) {throw 'Debug port already occupied.'}
$dbRoot=(Resolve-Path -LiteralPath $DatabaseDirectory).Path
$artRoot=(Resolve-Path -LiteralPath $ArtworkDirectory).Path
$results=[IO.Path]::GetFullPath($ResultsDirectory)
if (Test-Path -LiteralPath $results) {throw 'Choose a NEW results directory.'}
New-Item -ItemType Directory -Path $results | Out-Null
$names=@('GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_BENCHMARK_ARTWORK','GAME_MANAGER_SYNTHETIC_ROOT','GAME_MANAGER_ARTWORK_WORKERS','GAME_MANAGER_BACKGROUND_POLICY','GAME_MANAGER_DATA_DIR','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','WEBVIEW2_USER_DATA_FOLDER')
$saved=@{}
foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
function Get-AppMemory([int]$RootId){
    $all=Get-CimInstance Win32_Process;$ids=@($RootId)
    do{$children=@($all | Where-Object {$_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids} | ForEach-Object {[int]$_.ProcessId});$ids+=$children}while($children.Count -gt 0)
    $live=@(Get-Process -Id $ids -ErrorAction SilentlyContinue)
    return @{processes=$live.Count;privateBytes=($live|Measure-Object PrivateMemorySize64 -Sum).Sum;nativePrivateBytes=(Get-Process -Id $RootId).PrivateMemorySize64}
}
try{
    foreach($size in $Sizes){
        if($size -notin @(500,12320,15000,25000)){throw 'Unsupported size'}
        $canonical=Join-Path $dbRoot "synthetic-v1-seed-42-$size.sqlite3"
        $before=(Get-FileHash -LiteralPath $canonical).Hash
        foreach($case in $Cases){
            if($case -notin @('idle','cpu','hash','write','art1','art2','mixed0','mixed1','mixed2','priority')){throw 'Unsupported case'}
            $root=Join-Path $results "benchmarks/data/jobs-$size-$case"
            & $generator $root $size 42
            if($LASTEXITCODE -ne 0){throw 'Job fixture generation failed'}
            # New empty derived cache for every policy; only synthetic originals
            # share NTFS file data. No source file is opened for writing.
            $caseArt=Join-Path $results "benchmarks/data/artwork-$size-$case"
            foreach($directory in @('originals','thumbnails','temporary')) {New-Item -ItemType Directory -Path (Join-Path $caseArt $directory) | Out-Null}
            $sourceArt=Join-Path $artRoot "artwork-$size"
            Copy-Item -LiteralPath (Join-Path $sourceArt 'synthetic-artwork-v1.json') -Destination $caseArt
            foreach($file in Get-ChildItem -LiteralPath (Join-Path $sourceArt 'originals') -File) {
                if($file.Name -notmatch '^[0-9]{4}\.img$' -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Unexpected synthetic source'}
                New-Item -ItemType HardLink -Path (Join-Path $caseArt ('originals/'+$file.Name)) -Value $file.FullName | Out-Null
            }
            $workers=if($case -in @('art1','mixed1')){1}elseif($case -in @('art2','mixed2','priority')){2}else{0}
            $env:GAME_MANAGER_BENCHMARK_DB=$canonical
            $env:GAME_MANAGER_BENCHMARK_ARTWORK=$caseArt
            $env:GAME_MANAGER_SYNTHETIC_ROOT=$root
            $env:GAME_MANAGER_ARTWORK_WORKERS=[string]$workers
            $env:GAME_MANAGER_BACKGROUND_POLICY=if($case -eq 'priority'){'foreground'}else{'unthrottled'}
            $env:GAME_MANAGER_DATA_DIR=Join-Path $results 'application-fallback'
            $env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $results "profile-$size-$case"
            $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
            $restarts=if($RestartCheck){2}else{1}
            for($restart=1;$restart -le $restarts;$restart++){
                $output=Join-Path $results "$size-$case-$restart.json"
                $appProcess=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru -RedirectStandardError "$output.app.stderr.txt"
                $null=$appProcess.Handle
                try{
                    $nodeArguments=@(('"'+(Join-Path $PSScriptRoot 'frontend-background.mjs')+'"'),'http://127.0.0.1:9223',('"'+$output+'"'),"--case=$case","--iterations=$Iterations","--cycles=$Cycles")
                    if($Trace){$nodeArguments+='--trace'}
                    if($TraceSort){$nodeArguments+='--trace-sort'}
                    if($ClosedDiagnostics){$nodeArguments+='--closed-diagnostics'}
                    if($ShutdownJob -and $restart -eq 1){$nodeArguments+='--shutdown-job'}
                    if($RestartCheck -and $restart -eq 2){$nodeArguments+='--restart-check'}
                    $node=Start-Process -FilePath (Get-Command node).Source -ArgumentList $nodeArguments -WindowStyle Hidden -PassThru -RedirectStandardOutput "$output.stdout.txt" -RedirectStandardError "$output.stderr.txt"
                    $null=$node.Handle;$series=[Collections.Generic.List[object]]::new()
                    do{$exited=$node.WaitForExit(2000);if($SampleMemory){$series.Add(@{utc=[DateTime]::UtcNow.ToString('o');memory=(Get-AppMemory $appProcess.Id)})}}while(!$exited)
                    $node.WaitForExit()
                    if($node.ExitCode -ne 0){throw "Native background scenario failed: $output"}
                    ConvertTo-Json -InputObject $series.ToArray() -Depth 5 | Set-Content -LiteralPath "$output.memory-series.json"
                    Get-AppMemory $appProcess.Id | ConvertTo-Json | Set-Content -LiteralPath "$output.memory.json"
                }finally{
                    if(!$appProcess.HasExited){$null=$appProcess.CloseMainWindow()}
                    if(!$appProcess.WaitForExit(10000)){throw 'App did not finish safe shutdown'}
                    if($appProcess.ExitCode -ne 0){throw 'App reported a failed shutdown'}
                }
                Write-Output "$size $case run $restart passed"
            }
            & $generator verify $root | Set-Content -LiteralPath (Join-Path $results "$size-$case.audit.json") -Encoding utf8
            if($LASTEXITCODE -ne 0){throw 'Synthetic database integrity/audit failed'}
            $audit=Get-Content -LiteralPath (Join-Path $results "$size-$case.audit.json") -Raw | ConvertFrom-Json
            if($audit.interrupted -ne 0 -or $audit.committed -ne $audit.items){throw 'Unexpected interrupted/inconsistent job checkpoint'}
        }
        if((Get-FileHash -LiteralPath $canonical).Hash -ne $before){throw 'Canonical database changed!'}
    }
}finally{foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}

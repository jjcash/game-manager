# SPDX-License-Identifier: GPL-3.0-or-later
param(
    [Parameter(Mandatory=$true)][string]$ResultsDirectory,
    [Parameter(Mandatory=$true)][ValidateSet(96,120,144)][int]$ExpectedDpi,
    [Parameter(Mandatory=$true)][string]$BenchmarkExecutable,
    [Parameter(Mandatory=$true)][string]$DatabaseDirectory,
    [int[]]$Sizes=@(15000,25000)
)
$ErrorActionPreference='Stop'
if(Get-Process game-manager -ErrorAction SilentlyContinue){throw 'Close existing Game Manager windows'}
if(Get-NetTCPConnection -LocalPort 9223 -State Listen -ErrorAction SilentlyContinue){throw 'Debug port occupied'}
$exe=(Resolve-Path -LiteralPath $BenchmarkExecutable).Path
$dbRoot=(Resolve-Path -LiteralPath $DatabaseDirectory).Path
$out=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $out){throw 'Choose new results directory'}
New-Item -ItemType Directory -Path $out | Out-Null
Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class PhysicalScalingDpi { [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h); }' -ErrorAction SilentlyContinue
$names=@('GAME_MANAGER_BENCHMARK_DB','GAME_MANAGER_BENCHMARK_ARTWORK','GAME_MANAGER_SYNTHETIC_ROOT','GAME_MANAGER_DATA_DIR','GAME_MANAGER_ARTWORK_WORKERS','GAME_MANAGER_BACKGROUND_POLICY','GAME_MANAGER_STARTUP_TIMING','GAME_MANAGER_STARTUP_PROFILE_WINDOW','WEBVIEW2_USER_DATA_FOLDER','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS')
$saved=@{}
foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
try {
    $env:GAME_MANAGER_ARTWORK_WORKERS=$null
    $env:GAME_MANAGER_BACKGROUND_POLICY=$null
    $env:GAME_MANAGER_STARTUP_TIMING=$null
    $env:GAME_MANAGER_STARTUP_PROFILE_WINDOW=$null
    foreach($size in $Sizes){
        if($size -notin @(15000,25000)){throw 'Unsupported size'}
        $db=Join-Path $dbRoot "synthetic-v1-seed-42-$size.sqlite3"
        $hash=(Get-FileHash -LiteralPath $db).Hash
        $env:GAME_MANAGER_BENCHMARK_DB=$db
        $env:GAME_MANAGER_BENCHMARK_ARTWORK=$null
        $env:GAME_MANAGER_SYNTHETIC_ROOT=$null
        $env:GAME_MANAGER_DATA_DIR=Join-Path $out "isolated-$size"
        $env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $out "profile-$size"
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-debugging-address=127.0.0.1'
        $appProcess=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru -RedirectStandardError "$out/$size.stderr.txt"
        $null=$appProcess.Handle
        try {
            & node "$PSScriptRoot/frontend-query.mjs" http://127.0.0.1:9223 "$out/$size-query.json"
            if($LASTEXITCODE -ne 0){throw 'Physical-scale native query regression failed'}
            $appProcess.Refresh()
            $dpi=[PhysicalScalingDpi]::GetDpiForWindow($appProcess.MainWindowHandle)
            @{dpi=$dpi;expectedDpi=$ExpectedDpi;databaseHash=$hash;executableHash=(Get-FileHash -LiteralPath $exe).Hash;emulated=$false} | ConvertTo-Json | Set-Content -Encoding utf8 "$out/$size-native.json"
            if($dpi -ne $ExpectedDpi){throw "Actual per-window DPI $dpi does not match $ExpectedDpi"}
            & node "$PSScriptRoot/physical-scaling.mjs" "$out/$size-scaling.json" ([string]($ExpectedDpi/96))
            if($LASTEXITCODE -ne 0){throw 'Physical-scale geometry/font audit failed'}
        } finally {
            if(-not $appProcess.HasExited){$null=$appProcess.CloseMainWindow();if(-not $appProcess.WaitForExit(15000)){throw 'Native app did not close safely'}}
            if((Get-FileHash -LiteralPath $db).Hash -ne $hash){throw 'Source database changed'}
        }
    }
} finally {
    foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}
}

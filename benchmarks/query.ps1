# SPDX-License-Identifier: GPL-3.0-or-later
param(
    [Parameter(Mandatory=$true)][string]$DatabaseDirectory,
    [Parameter(Mandatory=$true)][string]$ResultsDirectory,
    [Parameter(Mandatory=$true)][string[]]$Executables,
    [int[]]$Sizes=@(500,12320,15000,25000),
    [ValidateRange(1,10000)][int]$Iterations=50,
    [ValidateRange(1,10)][int]$Passes=2,
    [switch]$Diagnose,
    [ValidateSet('all','finalists','relationships')][string]$Strategies='all'
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath($ResultsDirectory)
if(Test-Path -LiteralPath $root){throw 'Choose a NEW results directory'}
$databaseRoot=(Resolve-Path -LiteralPath $DatabaseDirectory).Path
$binaries=@($Executables | ForEach-Object {(Resolve-Path -LiteralPath $_).Path})
New-Item -ItemType Directory -Path $root | Out-Null
$environment=@{
    utc=[DateTime]::UtcNow.ToString('o'); commit=(& git rev-parse HEAD)
    build='Rust release opt-level=3 LTO one codegen unit; benchmarks feature; locked dependencies'
    cache='OS cache retained; one initial count/page per case; no concurrent builds/tests. Reverse artifact order each pass.'
    cpu=(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
    os=(Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,TotalVisibleMemorySize)
    disks=(Get-PhysicalDisk | Select-Object FriendlyName,MediaType,BusType)
    binaries=@($binaries | ForEach-Object {@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash}})
    sizes=$Sizes;iterations=$Iterations;passes=$Passes;diagnose=[bool]$Diagnose;strategies=$Strategies
}
$environment | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 -LiteralPath (Join-Path $root 'environment.json')
for($pass=0;$pass -lt $Passes;$pass++){
    foreach($size in $Sizes){
        if($size -notin @(500,2000,5000,12320,15000,25000)){throw 'Unsupported synthetic size'}
        $database=Join-Path $databaseRoot "synthetic-v1-seed-42-$size.sqlite3"
        $before=(Get-FileHash -LiteralPath $database).Hash
        $order=0..($binaries.Count-1)
        if($pass%2 -eq 1){[array]::Reverse($order)}
        foreach($index in $order){
            $action=if($Diagnose){'query-diagnose'}else{'query-measure'}
            $output=Join-Path $root "$size-pass-$pass-artifact-$index.json"
            $options=@('--database',$database,'--iterations',[string]$Iterations)
            if($Diagnose){$options+=@('--strategies',$Strategies)}
            & $binaries[$index] $action @options 2> "$output.stderr.txt" | Set-Content -Encoding utf8 -LiteralPath $output
            if($LASTEXITCODE -ne 0){throw "Query benchmark failed: $output"}
            if((Get-FileHash -LiteralPath $database).Hash -ne $before){throw 'Source database changed'}
            Write-Output "$size pass $pass artifact $index passed"
        }
        @{path=$database;sha256=$before;bytes=(Get-Item -LiteralPath $database).Length} | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath (Join-Path $root "$size-source.json")
    }
}

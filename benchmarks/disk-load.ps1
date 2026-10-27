# SPDX-License-Identifier: GPL-3.0-or-later
# A bounded, explicit synthetic disk contender. Never scans or deletes files.
param(
    [Parameter(Mandatory=$true)][string]$Root,
    [ValidateRange(1,600)][int]$DurationSeconds=180,
    [ValidateRange(64,1024)][int]$FileMiB=256
)
$ErrorActionPreference='Stop'
if($Root -notmatch '^[A-Za-z]:[\\/]'){throw 'Use an absolute local drive fixture root.'}
$path=[IO.Path]::GetFullPath($Root)
$directory=[IO.DirectoryInfo]::new($path)
if(!$directory.Name.StartsWith('disk-') -or $directory.Parent.Name -ne 'data' -or $directory.Parent.Parent.Name -ne 'benchmarks'){throw 'Use a NEW benchmarks/data/disk-NAME root.'}
if(Test-Path -LiteralPath $path){throw 'Existing root preserved; choose a new root.'}
$parent=$directory.Parent
while($parent){
    if((Test-Path -LiteralPath $parent.FullName) -and ((Get-Item -LiteralPath $parent.FullName).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Reparse ancestor refused.'}
    $parent=$parent.Parent
}
New-Item -ItemType Directory -Path $path | Out-Null
$marker=Join-Path $path 'synthetic-disk-load-v1.json'
@{purpose='synthetic-disk-load';version=1;fileMiB=$FileMiB;durationSeconds=$DurationSeconds;stopFile='stop.request';startedUtc=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath $marker -Encoding utf8
$buffer=New-Object byte[] (1024*1024)
[Random]::new(42).NextBytes($buffer)
$file=Join-Path $path 'payload.bin'
$stream=[IO.FileStream]::new($file,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None,$buffer.Length,[IO.FileOptions]::WriteThrough)
$clock=[Diagnostics.Stopwatch]::StartNew()
[long]$written=0
$started=[DateTime]::UtcNow.ToString('o')
try{
    while($clock.Elapsed.TotalSeconds -lt $DurationSeconds -and !(Test-Path -LiteralPath (Join-Path $path 'stop.request'))){
        $stream.Write($buffer,0,$buffer.Length)
        $written+=$buffer.Length
        if($stream.Position -ge [long]$FileMiB*1024*1024){$stream.Position=0}
    }
    $stream.Flush($true)
}finally{$stream.Dispose()}
@{purpose='synthetic-disk-load';root=$path;file=$file;startedUtc=$started;endedUtc=[DateTime]::UtcNow.ToString('o');elapsedMs=$clock.Elapsed.TotalMilliseconds;bytesWritten=$written;fileBytes=(Get-Item -LiteralPath $file).Length;writeThrough=$true;stoppedOnRequest=(Test-Path -LiteralPath (Join-Path $path 'stop.request'))} | ConvertTo-Json

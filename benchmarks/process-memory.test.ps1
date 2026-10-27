# SPDX-License-Identifier: GPL-3.0-or-later
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/process-memory.ps1"
$at = [DateTime]::Parse('2026-10-27T00:00:00Z').ToUniversalTime()
$snapshot = @(
    [pscustomobject]@{ProcessId=10;ParentProcessId=1;CreationDate=$at},
    [pscustomobject]@{ProcessId=20;ParentProcessId=10;CreationDate=$at.AddSeconds(1)},
    [pscustomobject]@{ProcessId=30;ParentProcessId=20;CreationDate=$at.AddSeconds(2)},
    [pscustomobject]@{ProcessId=40;ParentProcessId=10;CreationDate=$at.AddSeconds(-5)},
    [pscustomobject]@{ProcessId=50;ParentProcessId=40;CreationDate=$at.AddSeconds(-4)},
    [pscustomobject]@{ProcessId=60;ParentProcessId=20;CreationDate=$at.AddSeconds(-1)}
)
$ids = @(Select-AppProcessTree $snapshot 10 | ForEach-Object ProcessId | Sort-Object)
if (($ids -join ',') -ne '10,20,30') { throw "Incorrect live ancestry: $ids" }
$rejected = $false
try { Select-AppProcessTree $snapshot 999 | Out-Null } catch { $rejected = $true }
if (!$rejected) { throw 'Missing root accepted' }
$memory = Get-AppMemory $PID
if ($memory.processes -lt 1 -or $memory.privateBytes -le 0 -or !($memory.samples | Where-Object id -eq $PID)) { throw 'Live process serialization failed' }
'Process ancestry and missing-root checks passed.'

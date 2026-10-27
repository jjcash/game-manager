# SPDX-License-Identifier: GPL-3.0-or-later
# ParentProcessId can refer to a departed process whose PID has been reused.
# A live child cannot be older than the parent in the sampled process tree.
function Select-AppProcessTree([object[]]$Processes, [int]$RootId) {
    $root = $Processes | Where-Object ProcessId -eq $RootId | Select-Object -First 1
    if (!$root) { throw 'Native root missing from process snapshot.' }
    $selected = @($root)
    do {
        $ids = @($selected | ForEach-Object { [int]$_.ProcessId })
        $children = @($Processes | Where-Object {
            $child = $_
            if ($child.ProcessId -in $ids -or $child.ParentProcessId -notin $ids) { return $false }
            $parent = $selected | Where-Object ProcessId -eq $child.ParentProcessId | Select-Object -First 1
            return $child.CreationDate -ge $parent.CreationDate
        })
        $selected += $children
    } while ($children.Count -gt 0)
    return $selected
}

function Get-AppMemory([int]$RootId) {
    $tree = @(Select-AppProcessTree @(Get-CimInstance Win32_Process) $RootId)
    $samples = @()
    foreach ($entry in $tree) {
        $live = Get-Process -Id $entry.ProcessId -ErrorAction SilentlyContinue
        if (!$live) { continue }
        # Also reject replacement processes created between the two snapshots.
        if ([math]::Abs(($live.StartTime.ToUniversalTime() - $entry.CreationDate.ToUniversalTime()).TotalMilliseconds) -gt 1) { continue }
        $role=if($entry.CommandLine -match '--type=([^ "\s]+)'){$Matches[1]}elseif($entry.ProcessId -eq $RootId){'native'}else{'browser'}
        $samples += [pscustomobject]@{ id=$live.Id; parentId=$entry.ParentProcessId; name=$live.ProcessName; role=$role; startedUtc=$live.StartTime.ToUniversalTime().ToString('o'); privateBytes=$live.PrivateMemorySize64; workingSetBytes=$live.WorkingSet64 }
    }
    $native = $samples | Where-Object id -eq $RootId
    if (!$native) { throw 'Native root exited or changed during memory capture.' }
    return @{ processes=$samples.Count; privateBytes=($samples | Measure-Object privateBytes -Sum).Sum; workingSetBytes=($samples | Measure-Object workingSetBytes -Sum).Sum; nativePrivateBytes=$native.privateBytes; samples=$samples }
}

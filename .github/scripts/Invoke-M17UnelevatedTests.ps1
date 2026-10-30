# SPDX-License-Identifier: GPL-3.0-or-later
# Fixed-purpose M1.7 CI feasibility runner; no command/target parameters.
$ErrorActionPreference = 'Stop'
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$temporaryParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$scratch = Join-Path $temporaryParent ('game-manager-m17-ci-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$exitCode = 1

function Invoke-OwnedPhase($FilePath, $Arguments, $Seconds, $Label) {
    $stdout = Join-Path $scratch ($Label + '.stdout.txt')
    $stderr = Join-Path $scratch ($Label + '.stderr.txt')
    $process = $null
    $confirmed = $false
    try {
        $start = @{
            FilePath = $FilePath
            WorkingDirectory = (Join-Path $repository 'apps/desktop')
            WindowStyle = 'Hidden'
            PassThru = $true
            RedirectStandardOutput = $stdout
            RedirectStandardError = $stderr
        }
        if ($Arguments) { $start.ArgumentList = $Arguments }
        $process = Start-Process @start
        # Keep the native process association alive for short-lived compiler exits.
        $null = $process.Handle
        if (-not $process.WaitForExit($Seconds * 1000)) {
            $script:exitCode = 124
            Write-Host "TIMEOUT: $Label after $Seconds seconds; terminating only owned PID $($process.Id)."
            $process.Kill()
            $confirmed = $process.WaitForExit(30000)
            if (-not $confirmed) { throw "Termination not confirmed: $Label" }
            return 124
        }
        $confirmed = $true
        if ($null -eq $process.ExitCode) { throw "Missing child exit code: $Label" }
        $script:exitCode = $process.ExitCode
        Write-Host "$Label actual child exit code: $($process.ExitCode)"
        return $process.ExitCode
    } finally {
        $cleanupFailure = $null
        if ($null -ne $process) {
            try {
                if (-not $confirmed) {
                    if (-not $process.HasExited) { $process.Kill() }
                    $confirmed = $process.WaitForExit(30000)
                    if (-not $confirmed) { throw "Termination not confirmed: $Label" }
                }
            } catch {
                Write-Host "CLEANUP FAILURE ($Label): $_"
                $cleanupFailure = $_
            } finally {
                if ($confirmed) { $process.Dispose() }
            }
        }
        # Regular files in our own scratch directory; no waits for descendant EOF.
        foreach ($log in @($stdout, $stderr)) {
            if (Test-Path -LiteralPath $log -PathType Leaf) {
                if ((Get-Item -LiteralPath $log).Length -gt 16MB) { throw "Output limit exceeded: $Label" }
                Get-Content -LiteralPath $log | Write-Host
            }
        }
        if ($null -ne $cleanupFailure) { throw $cleanupFailure }
    }
}

function Remove-OwnedScratch {
    $resolved = [IO.Path]::GetFullPath($scratch)
    if (-not [string]::Equals([IO.Path]::GetDirectoryName($resolved), $temporaryParent, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($resolved) -notmatch '^game-manager-m17-ci-[0-9a-f]{32}$') {
        throw 'Refusing cleanup outside the owned M1.7 scratch directory.'
    }
    # Check every entry before any deletion. Never traverse a reparse point.
    $pending = [Collections.Generic.Stack[string]]::new()
    $pending.Push($resolved)
    while ($pending.Count -gt 0) {
        $directory = $pending.Pop()
        $item = Get-Item -LiteralPath $directory -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing scratch reparse-point cleanup.' }
        foreach ($child in Get-ChildItem -LiteralPath $directory -Force) {
            if ($child.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing scratch reparse-point cleanup.' }
            if ($child.PSIsContainer) { $pending.Push($child.FullName) }
        }
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
    if (Test-Path -LiteralPath $resolved) { throw 'Scratch deletion did not complete.' }
}

try {
    # Windows' existing .NET Framework compiler; no dependency download/install.
    $compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) {
        throw 'The Windows .NET Framework compiler prerequisite is missing.'
    }
    $executable = Join-Path $scratch 'M17UnelevatedTests.exe'
    $source = Join-Path $PSScriptRoot 'M17UnelevatedTests.cs'
    $exitCode = Invoke-OwnedPhase $compiler "/nologo /warnaserror+ /out:`"$executable`" `"$source`"" 120 'helper-compilation'
    if ($exitCode -eq 0) {
        $exitCode = Invoke-OwnedPhase $executable $null (44 * 60) 'restricted-helper'
    }
} catch {
    Write-Host "M1.7 wrapper failure: $_"
    if ($exitCode -eq 0) { $exitCode = 1 }
} finally {
    Write-Host "M1.7 original child/test result: $exitCode"
    try {
        Remove-OwnedScratch
        Write-Host 'M1.7 CI scratch cleanup completed.'
    } catch {
        Write-Host "M1.7 CI scratch cleanup FAILED (original result $exitCode): $_"
        if ($exitCode -eq 0) { $exitCode = 1 }
    }
}
exit $exitCode

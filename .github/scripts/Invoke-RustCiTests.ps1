# SPDX-License-Identifier: GPL-3.0-or-later
# Fixed Cargo target partition; never use a libtest name filter for hosted CI.
param(
    [Parameter(Mandatory)]
    [ValidateSet('hosted')]
    [string]$Suite
)
$ErrorActionPreference = 'Stop'
$manifest = Join-Path $PSScriptRoot '../../apps/desktop/src-tauri/Cargo.toml'
$common = @('--manifest-path', $manifest, '--all-features', '--locked', '--color', 'never')

function Invoke-Cargo([string[]]$CargoArguments) {
    & cargo @CargoArguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

if ($Suite -eq 'hosted') {
    $json = & cargo metadata --manifest-path $manifest --no-deps --format-version 1 --all-features --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $metadata = $json | ConvertFrom-Json
    $package = @($metadata.packages | Where-Object { $_.name -eq 'game-manager' })
    if ($package.Count -ne 1) { throw 'Expected exactly one game-manager package.' }
    $targets = @($package[0].targets | Where-Object { $_.test -and $_.kind -contains 'test' })
    $nativeTargets = @('standalone_launch', 'standalone_launch_production')
    foreach ($nativeTarget in $nativeTargets) {
        if (@($targets | Where-Object { $_.name -eq $nativeTarget }).Count -ne 1) {
            throw "Mandatory native target missing: $nativeTarget"
        }
    }
    $selection = @('--lib', '--bins')
    foreach ($target in $targets) {
        if ($target.name -notin $nativeTargets) { $selection += @('--test', $target.name) }
    }
    # Compile native assertions on hosted Windows; execute them only under the local gate.
    foreach ($nativeTarget in $nativeTargets) {
        Invoke-Cargo (@('test') + $common + @('--test', $nativeTarget, '--no-run'))
    }
    Invoke-Cargo (@('test') + $common + $selection)
    Invoke-Cargo (@('test') + $common + @('--doc'))
    exit 0
}


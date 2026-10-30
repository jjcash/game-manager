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
    if (@($targets | Where-Object { $_.name -eq 'standalone_launch' }).Count -ne 1) {
        throw 'The mandatory standalone_launch target is missing.'
    }
    $selection = @('--lib', '--bins')
    foreach ($target in $targets) {
        if ($target.name -ne 'standalone_launch') { $selection += @('--test', $target.name) }
    }
    # Compile native assertions on hosted Windows; execute them only under the local gate.
    Invoke-Cargo (@('test') + $common + @('--test', 'standalone_launch', '--no-run'))
    Invoke-Cargo (@('test') + $common + $selection)
    Invoke-Cargo (@('test') + $common + @('--doc'))
    exit 0
}


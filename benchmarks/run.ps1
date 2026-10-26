# SPDX-License-Identifier: GPL-3.0-or-later
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DatabaseDirectory,
    [UInt64]$Seed = 42,
    [ValidateRange(1, 10000)][int]$Iterations = 50
)
$ErrorActionPreference = 'Stop'
if ($DatabaseDirectory -notmatch '^[A-Za-z]:[\\/]') {
    throw 'DatabaseDirectory must be an absolute local path.'
}
$manifest = Join-Path $PSScriptRoot '../apps/desktop/src-tauri/Cargo.toml'
& cargo build --manifest-path $manifest --bin library-bench --features benchmarks --release --locked
if ($LASTEXITCODE -ne 0) { throw 'Release benchmark build failed.' }
$executable = Join-Path $PSScriptRoot '../apps/desktop/src-tauri/target/release/library-bench.exe'
$reports = foreach ($size in @(500, 2000, 5000, 12320, 15000, 25000)) {
    $database = Join-Path $DatabaseDirectory "synthetic-v1-seed-$Seed-$size.sqlite3"
    $generation = & $executable generate --database $database --games $size --seed $Seed
    if ($LASTEXITCODE -ne 0) { throw "Generation failed for $size; existing files are never replaced. Use a new directory." }
    $measurement = & $executable measure --database $database --iterations $Iterations
    if ($LASTEXITCODE -ne 0) { throw "Measurement failed for $size." }
    [PSCustomObject]@{ generation = ($generation | ConvertFrom-Json); measurement = ($measurement | ConvertFrom-Json) }
}
ConvertTo-Json -InputObject @($reports) -Depth 20

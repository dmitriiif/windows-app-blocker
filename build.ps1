[CmdletBinding()]
param(
    # Also run the unit tests and clippy before building.
    [switch]$Check
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$appDirectory = Join-Path $PSScriptRoot 'app'
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $cargoHome 'bin'
    if (-not (Test-Path -LiteralPath (Join-Path $cargoBin 'cargo.exe'))) {
        throw 'Rust is not installed. Install it from https://rustup.rs and run this script again.'
    }
    $env:Path = "$cargoBin;$env:Path"
}

# The GNU toolchain keeps dlltool and its linker in a folder that is not on PATH by default.
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }
$selfContained = Get-ChildItem -Path (Join-Path $rustupHome 'toolchains') -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName 'lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained' } |
    Where-Object { Test-Path -LiteralPath $_ } |
    Select-Object -First 1
if ($selfContained) { $env:Path = "$selfContained;$env:Path" }

Push-Location $appDirectory
try {
    if ($Check) {
        cargo test --locked
        if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
        cargo clippy --locked --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Clippy reported problems.' }
    }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw "Build failed with exit code $LASTEXITCODE." }
} finally {
    Pop-Location
}

$outputPath = Join-Path $PSScriptRoot 'Windows App Blocker.exe'
Copy-Item -LiteralPath (Join-Path $appDirectory 'target\release\windows-app-blocker.exe') -Destination $outputPath -Force
Write-Output "Built $outputPath"

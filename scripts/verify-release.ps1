# Verify the SemASM 0.2 source tree before tagging a release.
[CmdletBinding()]
param(
    [string]$ExpectedVersion = "0.2.0"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$manifestVersion = Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"$' |
    Select-Object -First 1 |
    ForEach-Object { $_.Matches[0].Groups[1].Value }
if ($manifestVersion -ne $ExpectedVersion) {
    throw "workspace version '$manifestVersion' does not match '$ExpectedVersion'"
}

& cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw "formatting failed" }
& cargo clippy --workspace --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "clippy failed" }
& cargo test --workspace --all-features
if ($LASTEXITCODE -ne 0) { throw "tests failed" }
& cargo doc --workspace --no-deps
if ($LASTEXITCODE -ne 0) { throw "documentation build failed" }
& cargo package --workspace --no-verify --allow-dirty
if ($LASTEXITCODE -ne 0) { throw "source packaging failed" }
& cargo run -q -p semasm-cli -- --version
if ($LASTEXITCODE -ne 0) { throw "CLI version check failed" }
& cargo run -q -p semasm-cli -- status
if ($LASTEXITCODE -ne 0) { throw "CLI status check failed" }
& cargo run -q -p semasm-cli -- contract check fixtures/contracts/write_all.sem.toml
if ($LASTEXITCODE -ne 0) { throw "quickstart contract check failed" }

Write-Host "SemASM $ExpectedVersion release verification passed."

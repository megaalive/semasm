# One-command SemASM onboarding. Core checks are always required; external
# toolchain readiness is reported separately unless -StrictToolchain is used.
[CmdletBinding()]
param(
    [string]$Target = $(if ($IsWindows) { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }),
    [switch]$StrictToolchain
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

Write-Host "== SemASM core readiness =="
cargo run -q -p semasm-cli -- --version
if ($LASTEXITCODE -ne 0) { throw "CLI build/version check failed" }
cargo run -q -p semasm-cli -- status
if ($LASTEXITCODE -ne 0) { throw "capability status check failed" }
cargo run -q -p semasm-cli -- contract check fixtures/contracts/write_all.sem.toml
if ($LASTEXITCODE -ne 0) { throw "contract validation check failed" }

Write-Host ""
Write-Host "== Optional target toolchain readiness: $Target =="
cargo run -q -p semasm-cli -- target doctor $Target
$doctorExit = $LASTEXITCODE
if ($doctorExit -eq 0) {
    Write-Host "onboarding_result=core_ready,target_ready"
} else {
    Write-Host "onboarding_result=core_ready,target_unavailable"
    Write-Host "Core onboarding passed. Follow the install hints above for end-to-end execution."
    if ($StrictToolchain) {
        exit $doctorExit
    }
}

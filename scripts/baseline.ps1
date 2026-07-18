# Collect the SemASM stabilization baseline without changing tracked source files.
[CmdletBinding()]
param(
    [string]$OutputPath = "target/stabilization-baseline.txt"
)

$ErrorActionPreference = "Continue"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot
$resolvedOutput = Join-Path $repoRoot $OutputPath
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $resolvedOutput) | Out-Null

function Write-Section([string]$Title) {
    Add-Content -LiteralPath $resolvedOutput -Value "`n## $Title"
}

function Invoke-Recorded([string]$Label, [scriptblock]$Command) {
    Add-Content -LiteralPath $resolvedOutput -Value "`n### $Label"
    $output = & $Command 2>&1 | Out-String
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
    Add-Content -LiteralPath $resolvedOutput -Value $output.TrimEnd()
    Add-Content -LiteralPath $resolvedOutput -Value "[exit code: $exitCode]"
}

Set-Content -LiteralPath $resolvedOutput -Value "# SemASM stabilization baseline"
Write-Section "Repository and host"
Invoke-Recorded "Commit" { git rev-parse HEAD }
Invoke-Recorded "Working tree" { git status --short }
Invoke-Recorded "Host" { rustc -Vv }
Invoke-Recorded "Cargo" { cargo -V }

Write-Section "External tools"
$toolProbes = @(
    @{ Name = "nasm"; Args = @("--version") },
    @{ Name = "objdump"; Args = @("--version") },
    @{ Name = "ld"; Args = @("--version") },
    @{ Name = "cc"; Args = @("--version") },
    @{ Name = "clang"; Args = @("--version") },
    @{ Name = "link"; Args = @() },
    @{ Name = "lld-link"; Args = @("--version") },
    @{ Name = "qemu-aarch64"; Args = @("--version") },
    @{ Name = "qemu-riscv64"; Args = @("--version") },
    @{ Name = "cargo-deny"; Args = @("--version") }
)
foreach ($probe in $toolProbes) {
    $tool = Get-Command $probe.Name -ErrorAction SilentlyContinue
    if ($null -eq $tool) {
        Add-Content -LiteralPath $resolvedOutput -Value "$($probe.Name): not found"
    } else {
        Add-Content -LiteralPath $resolvedOutput -Value "$($probe.Name): $($tool.Source)"
        Invoke-Recorded "$($probe.Name) version" { & $tool.Source @($probe.Args) }
    }
}

Write-Section "Ignored tests"
Get-ChildItem -Path crates -Recurse -Filter *.rs |
    Select-String -Pattern '#\[ignore' |
    ForEach-Object { "$($_.Path.Substring($repoRoot.Length + 1)):$($_.LineNumber): $($_.Line.Trim())" } |
    Add-Content -LiteralPath $resolvedOutput

Write-Section "Fixtures"
Get-ChildItem -Path fixtures -Recurse -File |
    ForEach-Object { $_.FullName.Substring($repoRoot.Length + 1).Replace('\', '/') } |
    Sort-Object |
    Add-Content -LiteralPath $resolvedOutput

Write-Section "Acceptance commands"
$checks = @(
    @{ Label = "Formatting"; Command = { cargo fmt --all -- --check } },
    @{ Label = "Clippy all features"; Command = { cargo clippy --workspace --all-targets --all-features -- -D warnings } },
    @{ Label = "Tests default features"; Command = { cargo test --workspace } },
    @{ Label = "Tests all features"; Command = { cargo test --workspace --all-features } },
    @{ Label = "Documentation"; Command = { cargo doc --workspace --no-deps } },
    @{ Label = "Dependency policy"; Command = { cargo deny check --all-features } },
    @{ Label = "Check no default features"; Command = { cargo check --workspace --no-default-features } },
    @{ Label = "Tests no default features"; Command = { cargo test --workspace --no-default-features } },
    @{ Label = "MSRV check"; Command = { cargo +1.85 check --workspace --all-targets } },
    @{ Label = "MSRV tests no default features"; Command = { cargo +1.85 test --workspace --no-default-features } },
    @{ Label = "CLI version"; Command = { cargo run -q -p semasm-cli -- --version } },
    @{ Label = "CLI status"; Command = { cargo run -q -p semasm-cli -- status } },
    @{ Label = "CLI target doctor"; Command = { cargo run -q -p semasm-cli -- target doctor x86_64-unknown-linux-gnu } },
    @{ Label = "Debug CLI build"; Command = { cargo build -p semasm-cli } },
    @{ Label = "Release CLI build no default features"; Command = { cargo build -p semasm-cli --no-default-features --release } },
    @{ Label = "Git diff check"; Command = { git diff --check } }
)
foreach ($check in $checks) {
    Invoke-Recorded $check.Label $check.Command
}

Write-Section "Binary sizes"
foreach ($binary in @("target/debug/semasm.exe", "target/release/semasm.exe")) {
    if (Test-Path -LiteralPath $binary) {
        $item = Get-Item -LiteralPath $binary
        Add-Content -LiteralPath $resolvedOutput -Value "$($binary.Replace('\', '/')): $($item.Length) bytes"
    } else {
        Add-Content -LiteralPath $resolvedOutput -Value "$($binary.Replace('\', '/')): not produced"
    }
}

Write-Host "Baseline written to $resolvedOutput"

# Golden-path demo: count_byte correct + wrong via agent verify.
# Default target is Win64 on this host; pass -SysV to force Linux SysV (needs qemu).
param(
    [switch]$SysV
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$target = if ($SysV) { "x86_64-unknown-linux-gnu" } else { "x86_64-pc-windows-msvc" }
$sourceOk = if ($SysV) { "fixtures/asm/count_byte.asm" } else { "fixtures/asm/count_byte_win64.asm" }
$sourceWrong = if ($SysV) { "fixtures/asm/count_byte_wrong.asm" } else { "fixtures/asm/count_byte_wrong_win64.asm" }
$contract = "fixtures/contracts/count_byte.sem.toml"
$cardDir = Join-Path $env:TEMP ("semasm-golden-demo-" + $PID)
New-Item -ItemType Directory -Force -Path $cardDir | Out-Null

function Invoke-CargoSemasm {
    param([string[]]$SemasmArgs)
    $argList = New-Object System.Collections.Generic.List[string]
    foreach ($a in @("run", "-q", "-p", "semasm-cli", "--features", "capstone", "--") + $SemasmArgs) {
        [void]$argList.Add($a)
    }
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = "cargo"
    $quoted = foreach ($a in $argList) {
        if ($a -match "\s") { '"{0}"' -f $a } else { $a }
    }
    $psi.Arguments = [string]::Join(" ", $quoted)
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    $proc.WaitForExit()
    if ($stderr -match "toolchain incomplete") {
        Write-Host ("toolchain incomplete - run: cargo run -p semasm-cli -- target doctor {0}" -f $target)
        Write-Host $stderr
        exit 1
    }
    return @{ ExitCode = $proc.ExitCode; Stdout = $stdout; Stderr = $stderr }
}

function Invoke-Verify {
    param(
        [string]$Label,
        [string]$Source,
        [switch]$AllowExecution
    )

    Write-Host ("=== {0} ===" -f $Label)
    $card = Join-Path $cardDir (([IO.Path]::GetFileNameWithoutExtension($Source)) + ".md")
    $args = @(
        "agent", "verify", $Source, $contract,
        "--target", $target,
        "--format", "json",
        "--card", $card
    )
    if ($AllowExecution) {
        $args += "--allow-execution"
    }
    $result = Invoke-CargoSemasm -SemasmArgs $args
    $report = $result.Stdout | ConvertFrom-Json
    $n = 0
    if ($null -ne $report.behavior -and $null -ne $report.behavior.cases) {
        $n = @($report.behavior.cases).Count
    }
    Write-Host ("status={0} isolation={1} vectors={2} exit={3}" -f $report.status, $report.isolation, $n, $result.ExitCode)
    if (Test-Path $card) {
        Write-Host ("--- evidence card: {0} ---" -f $card)
        Get-Content $card -TotalCount 20 | ForEach-Object { Write-Host $_ }
    }
}

Invoke-Verify -Label "static gates only (expect execution_denied)" -Source $sourceOk
Invoke-Verify -Label "allow-execution correct (expect verified)" -Source $sourceOk -AllowExecution
Invoke-Verify -Label "allow-execution wrong (expect behavior_failed)" -Source $sourceWrong -AllowExecution

Write-Host "=== compare correct vs wrong ==="
$cmp = Invoke-CargoSemasm -SemasmArgs @(
    "agent", "compare", $sourceOk, $sourceWrong, $contract,
    "--target", $target, "--format", "json", "--allow-execution"
)
$report = $cmp.Stdout | ConvertFrom-Json
Write-Host ("status_a={0} status_b={1} preferred={2}" -f $report.status_a, $report.status_b, $report.preferred)

Write-Host ("Golden demo finished (target={0})." -f $target)

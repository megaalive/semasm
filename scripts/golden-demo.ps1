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

function Invoke-Verify {
    param(
        [string]$Label,
        [string]$Source,
        [switch]$AllowExecution
    )

    Write-Host ("=== {0} ===" -f $Label)
    $argList = New-Object System.Collections.Generic.List[string]
    foreach ($a in @(
            "run", "-q", "-p", "semasm-cli", "--features", "capstone", "--",
            "agent", "verify", $Source, $contract,
            "--target", $target,
            "--format", "json"
        )) {
        [void]$argList.Add($a)
    }
    if ($AllowExecution) {
        [void]$argList.Add("--allow-execution")
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
    $exit = $proc.ExitCode

    if ($stderr -match "toolchain incomplete") {
        Write-Host ("toolchain incomplete - run: cargo run -p semasm-cli -- target doctor {0}" -f $target)
        Write-Host $stderr
        exit 1
    }

    $report = $stdout | ConvertFrom-Json
    $n = 0
    if ($null -ne $report.behavior -and $null -ne $report.behavior.cases) {
        $n = @($report.behavior.cases).Count
    }
    Write-Host ("status={0} isolation={1} vectors={2} exit={3}" -f $report.status, $report.isolation, $n, $exit)
}

Invoke-Verify -Label "static gates only (expect execution_denied)" -Source $sourceOk
Invoke-Verify -Label "allow-execution correct (expect verified)" -Source $sourceOk -AllowExecution
Invoke-Verify -Label "allow-execution wrong (expect behavior_failed)" -Source $sourceWrong -AllowExecution

Write-Host ("Golden demo finished (target={0})." -f $target)

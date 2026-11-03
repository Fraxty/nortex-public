<#
.SYNOPSIS
  Reproduzierbarer Compiler-Benchmark: Kompilierzeit, Peak-RAM, EXE-Größe, Laufzeit.
.DESCRIPTION
  Baut jede .nx-Datei aus examples\ mit target\release\nortexc.exe mehrfach (Standard 5 Läufe),
  meldet Median und Minimum der Wandzeit, den Peak Working Set des Compilerprozesses (Abfrage per
  Polling, daher eine untere Schranke), die Größe der erzeugten EXE und die Laufzeit des Programms.
  Dateien, die der aktuelle Compiler nicht übersetzt, werden als FEHLGESCHLAGEN aufgeführt und nicht
  in die Messung einbezogen. Es werden keine Werte geschätzt.
.EXAMPLE
  scripts\bench_compile.ps1 -Runs 5 -OutFile docs\release\BENCHMARKS.md
#>
param(
    [int]$Runs = 5,
    [string]$Compiler = "target\release\nortexc.exe",
    [string]$OutFile = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$work = Join-Path $env:TEMP "nortex-bench"
New-Item -ItemType Directory -Force $work | Out-Null

function Invoke-Timed([string]$exe, [string[]]$arguments) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $exe
    $info.Arguments = ($arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $process = [System.Diagnostics.Process]::Start($info)
    $outTask = $process.StandardOutput.ReadToEndAsync()
    $errTask = $process.StandardError.ReadToEndAsync()
    $peak = 0L
    while (-not $process.HasExited) {
        try { $process.Refresh(); $peak = [Math]::Max($peak, $process.PeakWorkingSet64) } catch {}
        Start-Sleep -Milliseconds 5
    }
    $process.WaitForExit()
    $watch.Stop()
    [pscustomobject]@{ Ms = $watch.Elapsed.TotalMilliseconds; PeakBytes = $peak; Exit = $process.ExitCode }
}

function Get-Median([double[]]$values) {
    $sorted = $values | Sort-Object
    $sorted[[int][Math]::Floor(($sorted.Count - 1) / 2)]
}

$rows = @()
$failed = @()
$sources = Get-ChildItem examples -Recurse -Filter *.nx | Sort-Object FullName
foreach ($source in $sources) {
    $rel = $source.FullName.Substring($root.Length + 1)
    $output = Join-Path $work (($rel -replace '[\\/:]', '_') + ".exe")
    $times = @(); $peaks = @(); $ok = $true
    for ($i = 0; $i -lt $Runs; $i++) {
        if (Test-Path $output) { Remove-Item $output -Force }
        $result = Invoke-Timed $Compiler @("build", $source.FullName, "--output", $output)
        if ($result.Exit -ne 0 -or -not (Test-Path $output)) { $ok = $false; break }
        $times += $result.Ms
        $peaks += $result.PeakBytes
    }
    if (-not $ok) { $failed += $rel; continue }
    $runMs = ""
    if ($rel -notmatch 'graphics|async') {
        $run = Invoke-Timed $output @()
        $runMs = "{0:N0}" -f $run.Ms
    }
    $rows += [pscustomobject]@{
        Datei = $rel
        MedianMs = [Math]::Round((Get-Median $times), 0)
        MinMs = [Math]::Round(($times | Measure-Object -Minimum).Minimum, 0)
        PeakMB = [Math]::Round((($peaks | Measure-Object -Maximum).Maximum) / 1MB, 1)
        ExeKB = [Math]::Round((Get-Item $output).Length / 1KB, 0)
        LaufMs = $runMs
    }
}

$lines = @()
$lines += "| Datei | Median ms | Min ms | Peak RAM MB | EXE KB | Lauf ms |"
$lines += "|---|---:|---:|---:|---:|---:|"
foreach ($row in $rows) {
    $lines += "| $($row.Datei) | $($row.MedianMs) | $($row.MinMs) | $($row.PeakMB) | $($row.ExeKB) | $($row.LaufMs) |"
}
if ($failed.Count -gt 0) {
    $lines += ""
    $lines += "Vom Compiler nicht übersetzt (nicht gemessen): " + ($failed -join ", ")
}
$report = $lines -join "`n"
$report
if ($OutFile) { [IO.File]::WriteAllText((Join-Path $root $OutFile), $report + "`n", (New-Object Text.UTF8Encoding $false)) }

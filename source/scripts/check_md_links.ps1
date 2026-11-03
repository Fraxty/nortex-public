$root = (Get-Location).Path
$files = git ls-files -co --exclude-standard '*.md' | Where-Object { $_ -notmatch '^(dist|target)/' }
foreach ($f in $files) {
  if (-not (Test-Path $f)) { continue }
  $dir = Split-Path (Resolve-Path $f) -Parent
  $inFence = $false; $n = 0
  foreach ($line in Get-Content $f -Encoding UTF8) {
    $n++
    if ($line -match '^\s*```') { $inFence = -not $inFence; continue }
    if ($inFence) { continue }
    foreach ($m in [regex]::Matches($line, '\]\(([^)\s]+)\)')) {
      $t = $m.Groups[1].Value
      if ($t -match '^(https?:|mailto:|#)') { continue }
      $p = ($t -split '#')[0]
      if ($p -eq '') { continue }
      $p = [uri]::UnescapeDataString($p)
      if (-not (Test-Path (Join-Path $dir $p))) { "$f`:$n -> $t" }
    }
  }
}

# Mark next pending Studio task and print it for loop agents.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$tasksPath = Join-Path $root "tasks.json"
$doc = Get-Content -Raw -Path $tasksPath | ConvertFrom-Json
$next = $doc.tasks | Where-Object { $_.status -eq "pending" } | Select-Object -First 1
if (-not $next) {
  Write-Output "ALL_DONE phase=$($doc.phase)"
  exit 0
}
foreach ($t in $doc.tasks) {
  if ($t.status -eq "in_progress") { $t.status = "pending" }
}
$next.status = "in_progress"
$doc.updatedAt = (Get-Date).ToString("yyyy-MM-dd")
$doc | ConvertTo-Json -Depth 8 | Set-Content -Path $tasksPath -Encoding UTF8
Write-Output "NEXT id=$($next.id) milestone=$($next.milestone) title=$($next.title)"

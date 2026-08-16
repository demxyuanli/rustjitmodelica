# Mark next pending Studio task and print it for loop agents.
# Usage:
#   pwsh -File ./build/modai_studio/run_next.ps1
#   pwsh -File ./build/modai_studio/run_next.ps1 -Board q4
param(
  [ValidateSet("p0", "p1", "p2", "p3", "q4")]
  [string]$Board = "q4"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$docsRoot = Join-Path (Split-Path -Parent (Split-Path -Parent $root)) "docs\modai-studio"
$fileMap = @{
  p0 = "tasks.json"
  p1 = "tasks_p1.json"
  p2 = "tasks_p2.json"
  p3 = "tasks_p3.json"
  q4 = "tasks_q4.json"
}
$name = $fileMap[$Board]
$tasksPath = Join-Path $root $name
$docsPath = Join-Path $docsRoot $name
# Q4: docs/ is canonical (build/ is gitignored). Always pull from docs when present.
if ($Board -eq "q4" -and (Test-Path $docsPath)) {
  Copy-Item -Path $docsPath -Destination $tasksPath -Force
}
if (-not (Test-Path $tasksPath)) {
  Write-Error "Missing task board: $tasksPath"
  exit 1
}
$doc = Get-Content -Raw -Path $tasksPath | ConvertFrom-Json
$next = $doc.tasks | Where-Object { $_.status -eq "pending" } | Select-Object -First 1
if (-not $next) {
  Write-Output "ALL_DONE phase=$($doc.phase) board=$Board"
  exit 0
}
foreach ($t in $doc.tasks) {
  if ($t.status -eq "in_progress") { $t.status = "pending" }
}
$next.status = "in_progress"
$doc.updatedAt = (Get-Date).ToString("yyyy-MM-dd")
$json = $doc | ConvertTo-Json -Depth 8
Set-Content -Path $tasksPath -Encoding UTF8 -Value $json
if ($Board -eq "q4" -and (Test-Path $docsRoot)) {
  Set-Content -Path $docsPath -Encoding UTF8 -Value $json
}
Write-Output "NEXT id=$($next.id) milestone=$($next.milestone) title=$($next.title)"

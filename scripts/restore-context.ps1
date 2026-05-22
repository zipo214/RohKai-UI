[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$snapshotPath = "D:\dev\rohkai\docs\context-snapshot.json"

if (-not (Test-Path $snapshotPath)) {
    Write-Host "No snapshot found at $snapshotPath. Run scripts/snapshot-context.ps1 first."
    exit 1
}

$s = Get-Content $snapshotPath -Raw | ConvertFrom-Json

Write-Host ""
Write-Host "=== RohKai Context Snapshot ==="
Write-Host "Captured : $($s.timestamp)"
Write-Host ""
Write-Host "Stage:"
Write-Host "  Last completed : $($s.lastCompletedStage)"
Write-Host "  Active         : $($s.activeStage)"
Write-Host ""
Write-Host "Last commit: $($s.lastCommit)"
Write-Host ""
$t = $s.tests
Write-Host "Tests: $($t.status) ($($t.passed) passed, $($t.failed) failed)"
Write-Host ""
$comments = @($s.citizenComments)
if ($comments.Count -eq 0) {
    Write-Host "CITIZEN comments: none"
} else {
    Write-Host "CITIZEN comments ($($comments.Count)):"
    foreach ($c in $comments) {
        Write-Host "  $($c.file):$($c.line) -- $($c.comment)"
    }
}
Write-Host ""
Write-Host "================================"

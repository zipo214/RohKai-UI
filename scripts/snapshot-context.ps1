Set-Location "D:\dev\rohkai"
$projectRoot = "D:\dev\rohkai"
$roadmapPath = "$projectRoot\docs\ROADMAP.md"
$outputPath  = "$projectRoot\docs\context-snapshot.json"
$checkmark   = [char]0x2705  # ✅ — PS5.1 regex can't match emoji literals reliably

# --- Stage from ROADMAP.md ---
$lastCompletedStage = ""
$activeStage        = ""
$roadmapLines = [System.IO.File]::ReadAllLines($roadmapPath, [System.Text.Encoding]::UTF8)
foreach ($line in $roadmapLines) {
    if ($line -match '^## (Stage \d+ .+)' -and $line.Contains($checkmark)) {
        $lastCompletedStage = $Matches[1].TrimEnd([char]' ', [char]0x2705)
    } elseif ($activeStage -eq "" -and $line -match '^## (Stage \d+ .+)' -and -not $line.Contains($checkmark)) {
        $activeStage = $Matches[1].TrimEnd()
    }
}

# --- Last git commit ---
$lastCommit = (git log --oneline -1).Trim()

# --- CITIZEN comments in src/ ---
$citizenComments = [System.Collections.Generic.List[object]]::new()
Get-ChildItem -Path "$projectRoot\src" -Recurse -Filter "*.rs" | ForEach-Object {
    $rel = $_.FullName.Substring($projectRoot.Length + 1)
    $n   = 0
    foreach ($line in [System.IO.File]::ReadAllLines($_.FullName, [System.Text.Encoding]::UTF8)) {
        $n++
        if ($line -match '//\s*CITIZEN:\s*(.+)') {
            $citizenComments.Add([PSCustomObject]@{ file = $rel; line = $n; comment = $Matches[1].Trim() })
        }
    }
}

# --- cargo test --quiet ---
$rawTest = cargo test --quiet 2>&1
$passed  = 0
$failed  = 0
foreach ($line in $rawTest) {
    if ($line -match 'test result:') {
        if ($line -match '(\d+) passed') { $passed += [int]$Matches[1] }
        if ($line -match '(\d+) failed') { $failed += [int]$Matches[1] }
    }
}
$testStatus = if ($failed -eq 0) { "ok" } else { "FAILING" }

# --- Write snapshot (UTF-8 no BOM via StreamWriter) ---
$snapshot = [ordered]@{
    timestamp          = (Get-Date -Format "yyyy-MM-ddTHH:mm:ss")
    lastCompletedStage = $lastCompletedStage
    activeStage        = $activeStage
    lastCommit         = $lastCommit
    citizenComments    = @($citizenComments)
    tests              = [ordered]@{ status = $testStatus; passed = $passed; failed = $failed }
}
$json = $snapshot | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($outputPath, $json, [System.Text.UTF8Encoding]::new($false))
Write-Host "Snapshot written: $outputPath"

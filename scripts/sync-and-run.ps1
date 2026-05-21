# Sync from Downloads working copy to D:\dev\rohkai, then run.
xcopy "D:\Users\zipo3\Downloads\Claude Code Projects\RohKai" "D:\dev\rohkai" /E /I /H /Y /EXCLUDE:scripts\xcopy-exclude.txt
Set-Location "D:\dev\rohkai"
cargo run

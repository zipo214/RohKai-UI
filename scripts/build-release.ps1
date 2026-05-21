# Build an optimised release executable.
Set-Location "D:\dev\rohkai"
cargo build --release
Write-Host "Binary at: D:\dev\rohkai\target\release\rohkai.exe"

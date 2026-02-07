$ErrorActionPreference = "Stop"

# --- CONFIGURATION ---
$DistDir = "dist"
$StagingDir = "staging_win"
$ZipName = "$DistDir\client_windows.zip"
# CHANGE: Path is now standard release folder, not inside a target subfolder
$ExePath = "target\release\ByAThread.exe" 
$SourceFile = "client\src\main.rs"

# --- SETUP DIRECTORIES ---
if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir | Out-Null
}

# --- EDIT SOURCE CODE (Fullscreen: true) ---
$OriginalContent = Get-Content -Path $SourceFile -Raw
if ($OriginalContent -match "fullscreen: false,") {
    Write-Host "Patching main.rs for fullscreen..."
    $NewContent = $OriginalContent -replace "fullscreen: false,", "fullscreen: true,"
    Set-Content -Path $SourceFile -Value $NewContent
}

try {
    # --- BUILD COMMAND ---
    # CHANGE: Removed '--target x86_64-pc-windows-gnu'
    Write-Host "Building client..."
    cargo build --release -p client

    # --- PREPARE STAGING ---
    if (Test-Path $StagingDir) {
        Remove-Item -Path $StagingDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $StagingDir | Out-Null

    # --- COPY FILES ---
    Write-Host "Copying assets..."
    Copy-Item -Path $ExePath -Destination "$StagingDir\"
    Copy-Item -Path "LICENSE" -Destination "$StagingDir\"
    Copy-Item -Path "CREDITS.md" -Destination "$StagingDir\"
    
    # Ensure parent dir exists for license
    $FontLicenseDest = "$StagingDir\NOTO_FONT_LICENSE.txt"
    Copy-Item -Path "client\assets\fonts\LICENSE.txt" -Destination $FontLicenseDest

    # --- ZIP IT UP ---
    if (Test-Path $ZipName) {
        Remove-Item -Path $ZipName -Force
    }
    Write-Host "Zipping to $ZipName..."
    Compress-Archive -Path "$StagingDir\*" -DestinationPath $ZipName

    # Cleanup staging
    Remove-Item -Path $StagingDir -Recurse -Force
    Write-Host "Done!"

} finally {
    # --- REVERT SOURCE CODE ---
    Write-Host "Reverting main.rs..."
    Set-Content -Path $SourceFile -Value $OriginalContent
}
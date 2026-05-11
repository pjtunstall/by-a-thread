$ErrorActionPreference = "Stop"

# --- CONFIGURATION ---
$DistDir = "dist"
$StagingDir = "ByAThread"
$Version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages | Where-Object { $_.name -eq "client" } | Select-Object -ExpandProperty version
if (-not $Version) {
    if ($env:GITHUB_ACTIONS -eq "true") {
        Write-Host "::warning::Could not determine client package version from cargo metadata; using 'unknown' in zip name."
    } else {
        Write-Warning "Could not determine client package version from cargo metadata; using 'unknown' in zip name."
    }
    $Version = "unknown"
}
Write-Host "Using client version: $Version"
$ZipName = "$DistDir\ByAThread-$Version-win64.zip"
$ExePath = "target\release\ByAThread.exe"

# --- SETUP DIRECTORIES ---
if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir | Out-Null
}

# --- BUILD COMMAND ---
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
Copy-Item -Path "LICENSE.txt" -Destination "$StagingDir\"
Copy-Item -Path "CREDITS.txt" -Destination "$StagingDir\"

New-Item -ItemType Directory -Path "$StagingDir\fonts" -Force | Out-Null
Copy-Item -Path "client\assets\fonts\macondo" -Destination "$StagingDir\fonts\" -Recurse
Copy-Item -Path "client\assets\fonts\noto" -Destination "$StagingDir\fonts\" -Recurse

# --- ZIP IT UP ---
if (Test-Path $ZipName) {
    Remove-Item -Path $ZipName -Force
}
Write-Host "Zipping to $ZipName..."
Compress-Archive -Path $StagingDir -DestinationPath $ZipName

Remove-Item -Path $StagingDir -Recurse -Force
Write-Host "Done!"

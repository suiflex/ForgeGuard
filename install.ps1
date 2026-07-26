$ErrorActionPreference = "Stop"

$Repository = "suiflex/ForgeGuard"
$Version = if ($env:FORGEGUARD_VERSION) { $env:FORGEGUARD_VERSION } else { "latest" }
if ($Version -notmatch '^(latest|v[0-9A-Za-z._-]+)$') {
    throw "Invalid FORGEGUARD_VERSION: $Version"
}

$Architecture = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x86_64" }
    "ARM64" { "aarch64" }
    default { throw "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
}

$Archive = "forgeguard-windows-$Architecture.zip"
$DownloadBase = if ($Version -eq "latest") {
    "https://github.com/$Repository/releases/latest/download"
} else {
    "https://github.com/$Repository/releases/download/$Version"
}

$TemporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ("forgeguard-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $TemporaryDirectory | Out-Null

try {
    $ArchivePath = Join-Path $TemporaryDirectory $Archive
    $ChecksumPath = "$ArchivePath.sha256"
    Invoke-WebRequest -UseBasicParsing -Uri "$DownloadBase/$Archive" -OutFile $ArchivePath
    Invoke-WebRequest -UseBasicParsing -Uri "$DownloadBase/$Archive.sha256" -OutFile $ChecksumPath

    $ExpectedChecksum = (Get-Content -Raw $ChecksumPath).Trim().ToLowerInvariant()
    $ActualChecksum = (Get-FileHash -Algorithm SHA256 $ArchivePath).Hash.ToLowerInvariant()
    if ($ExpectedChecksum -ne $ActualChecksum) {
        throw "Checksum verification failed"
    }

    Expand-Archive -Path $ArchivePath -DestinationPath $TemporaryDirectory -Force
    $InstallDirectory = if ($env:FORGEGUARD_INSTALL_DIR) {
        $env:FORGEGUARD_INSTALL_DIR
    } else {
        Join-Path $env:LOCALAPPDATA "ForgeGuard\bin"
    }
    New-Item -ItemType Directory -Force -Path $InstallDirectory | Out-Null
    $BinaryPath = Join-Path $InstallDirectory "forgeguard.exe"
    Copy-Item -Force (Join-Path $TemporaryDirectory "forgeguard.exe") $BinaryPath

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $PathEntries = @($UserPath -split ';' | Where-Object { $_ })
    if ($InstallDirectory -notin $PathEntries) {
        $UpdatedPath = (@($PathEntries) + $InstallDirectory) -join ';'
        [Environment]::SetEnvironmentVariable("Path", $UpdatedPath, "User")
    }
    $env:Path = "$InstallDirectory;$env:Path"

    & $BinaryPath init --global --agent all
    if ($LASTEXITCODE -ne 0) {
        throw "ForgeGuard global setup failed"
    }

    Write-Host ""
    Write-Host "ForgeGuard installed: $BinaryPath"
    Write-Host "Restart terminal, then run inside a repository:"
    Write-Host "  forgeguard init --agent all"
}
finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $TemporaryDirectory
}

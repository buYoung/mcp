<#
install.ps1 — codemap-search prebuilt-binary installer for Windows.

Detects Windows architecture (x64 or arm64), resolves the matching GitHub
Release .zip asset, verifies the sibling .sha256 BEFORE extracting, and
installs `codemap-search.exe` to a directory on PATH (default $HOME\.local\bin).

Usage:
  irm https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.ps1 | iex
  & ([scriptblock]::Create((irm https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.ps1))) -Version codemap-v0.1.6

Environment overrides:
  INSTALL_DIR            install target dir (default: $HOME\.local\bin)
  CODEMAP_VERSION        pin a release tag, e.g. codemap-v0.1.6 (default: latest)
  CODEMAP_OS_OVERRIDE    override detected OS name for dry-run checks
  CODEMAP_ARCH_OVERRIDE  override detected architecture for dry-run checks
#>

param(
    [string]$Version = "",
    [string]$InstallDir = "",
    [switch]$PrintTarget,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

& {
    $repo = "buYoung/mcp"
    $bin = "codemap-search"
    $exeName = "$bin.exe"

    if ($Help) {
        [Console]::Error.WriteLine("usage: install.ps1 [-Version <codemap-vX.Y.Z>] [-InstallDir <dir>] [-PrintTarget]")
        [Console]::Error.WriteLine("env: INSTALL_DIR, CODEMAP_VERSION")
        return
    }

    function Resolve-IsWindows {
        if ($env:CODEMAP_OS_OVERRIDE) {
            return ($env:CODEMAP_OS_OVERRIDE -eq "Windows_NT")
        }
        $isWinVar = Get-Variable -Name IsWindows -ErrorAction SilentlyContinue
        if ($null -ne $isWinVar) {
            return [bool]$isWinVar.Value
        }
        return ($env:OS -eq "Windows_NT")
    }

    function Resolve-Arch {
        $raw = $env:CODEMAP_ARCH_OVERRIDE
        if (-not $raw) {
            try {
                $raw = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            } catch {
                $raw = ""
            }
        }
        if (-not $raw) {
            $raw = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
        }
        switch -Regex ($raw) {
            '^(?i)(x64|amd64|x86_64)$' { return "x86_64" }
            '^(?i)(arm64|aarch64)$'    { return "aarch64" }
            default {
                throw "unsupported architecture: $raw (only x86_64 and arm64/aarch64 are released)"
            }
        }
    }

    if (-not (Resolve-IsWindows)) {
        $osName = if ($env:CODEMAP_OS_OVERRIDE) { $env:CODEMAP_OS_OVERRIDE } else { [System.Environment]::OSVersion.Platform.ToString() }
        throw "unsupported OS: $osName (install.ps1 covers Windows only; use install.sh on macOS and Linux)"
    }

    $arch = Resolve-Arch
    $target = switch ($arch) {
        "x86_64"  { "x86_64-pc-windows-msvc" }
        "aarch64" { "aarch64-pc-windows-msvc" }
    }
    $asset = "$bin-$target.zip"
    $shaAsset = "$bin-$target.zip.sha256"

    $resolvedVersion = if ($Version) { $Version } elseif ($env:CODEMAP_VERSION) { $env:CODEMAP_VERSION } else { "" }
    $base = if ($resolvedVersion) {
        "https://github.com/$repo/releases/download/$resolvedVersion"
    } else {
        "https://github.com/$repo/releases/latest/download"
    }
    $archiveUrl = "$base/$asset"
    $shaUrl = "$base/$shaAsset"

    $resolvedInstallDir = if ($InstallDir) {
        $InstallDir
    } elseif ($env:INSTALL_DIR) {
        $env:INSTALL_DIR
    } else {
        Join-Path $HOME ".local\bin"
    }

    if ($PrintTarget) {
        $osLabel = if ($env:CODEMAP_OS_OVERRIDE) { $env:CODEMAP_OS_OVERRIDE } else { "Windows_NT" }
        Write-Output "os:          $osLabel"
        Write-Output "arch:        $arch"
        Write-Output "target:      $target"
        Write-Output "asset:       $asset"
        Write-Output "archive_url: $archiveUrl"
        Write-Output "sha_url:     $shaUrl"
        Write-Output "install_dir: $resolvedInstallDir"
        return
    }

    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $ProgressPreference = "SilentlyContinue"

    $tmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("$bin-install-" + [System.Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $tmpRoot -Force | Out-Null
    $stageTmpDir = $null

    try {
        $archivePath = Join-Path $tmpRoot $asset
        $shaPath = "$archivePath.sha256"
        $extractDir = Join-Path $tmpRoot "extract"

        [Console]::Error.WriteLine("resolving $bin ($target)...")
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $archiveUrl -OutFile $archivePath
        } catch {
            throw "download failed: $archiveUrl ($($_.Exception.Message))"
        }
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $shaUrl -OutFile $shaPath
        } catch {
            throw "download failed: $shaUrl ($($_.Exception.Message))"
        }

        # Verify the sibling .sha256 BEFORE extracting anything.
        [Console]::Error.WriteLine("verifying sha256...")
        $shaContent = (Get-Content -LiteralPath $shaPath -Raw).Trim()
        if (-not $shaContent) {
            throw "checksum file is empty: $shaPath"
        }
        $expectedHash = ($shaContent -split '\s+')[0].ToLowerInvariant()
        if ($expectedHash -notmatch '^[0-9a-f]{64}$') {
            throw "checksum file is malformed: $shaPath"
        }
        $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($expectedHash -ne $actualHash) {
            throw "checksum mismatch (expected $expectedHash, got $actualHash) -- refusing to install"
        }

        Expand-Archive -LiteralPath $archivePath -DestinationPath $extractDir -Force
        $extractedBin = Join-Path $extractDir $exeName
        if (-not (Test-Path -LiteralPath $extractedBin -PathType Leaf)) {
            throw "$exeName not found in archive: $asset"
        }
        $binItem = Get-Item -LiteralPath $extractedBin -Force
        if (($binItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "$exeName in archive is a reparse point -- refusing to install"
        }

        if (-not (Test-Path -LiteralPath $resolvedInstallDir -PathType Container)) {
            New-Item -ItemType Directory -Path $resolvedInstallDir -Force | Out-Null
        }
        $resolvedInstallFull = (Get-Item -LiteralPath $resolvedInstallDir).FullName

        $stageTmpDir = Join-Path $resolvedInstallFull (".$bin.tmp." + [System.Guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $stageTmpDir -Force | Out-Null
        $stagedBin = Join-Path $stageTmpDir $exeName
        Copy-Item -LiteralPath $extractedBin -Destination $stagedBin -Force
        Move-Item -LiteralPath $stagedBin -Destination (Join-Path $resolvedInstallFull $exeName) -Force

        [Console]::Error.WriteLine("installed $bin to $(Join-Path $resolvedInstallFull $exeName)")

        $normTarget = $resolvedInstallFull.TrimEnd('\', '/').ToLowerInvariant()
        $pathEntries = ($env:PATH -split ';') | ForEach-Object { $_.Trim().TrimEnd('\', '/').ToLowerInvariant() }
        if ($pathEntries -notcontains $normTarget) {
            [Console]::Error.WriteLine("")
            [Console]::Error.WriteLine("note: $resolvedInstallFull is not on your PATH.")
            [Console]::Error.WriteLine("      add it for this session:  `$env:PATH = `"$resolvedInstallFull;`$env:PATH`"")
            [Console]::Error.WriteLine("      to persist across sessions: [Environment]::SetEnvironmentVariable(`"Path`", `"`$env:Path;$resolvedInstallFull`", `"User`")")
        }
    } finally {
        if ($stageTmpDir -and (Test-Path -LiteralPath $stageTmpDir)) {
            Remove-Item -LiteralPath $stageTmpDir -Recurse -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path -LiteralPath $tmpRoot) {
            Remove-Item -LiteralPath $tmpRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

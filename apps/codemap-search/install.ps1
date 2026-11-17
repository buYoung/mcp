<#
install.ps1 — codemap-search prebuilt-binary installer for Windows.

Detects Windows architecture (x64 or arm64), resolves the matching GitHub
Release .zip asset, verifies the sibling .sha256 BEFORE extracting, and
installs `codemap-search.exe` to a directory on PATH (default $HOME\.local\bin).

Usage:
  powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.ps1 | iex"
  & ([scriptblock]::Create((irm https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.ps1))) -Version 0.14.2

Environment overrides:
  INSTALL_DIR             install target dir (default: $HOME\.local\bin)
  CODEMAP_VERSION         pin a release tag or version (default: latest)
  CODEMAP_NO_MODIFY_PATH  set to 1/true/yes to skip updating User Path
  CODEMAP_OS_OVERRIDE     override detected OS name for dry-run checks
  CODEMAP_ARCH_OVERRIDE   override detected architecture for dry-run checks
#>

param(
    [string]$Version = "",
    [string]$InstallDir = "",
    [switch]$NoModifyPath,
    [switch]$PrintTarget,
    [switch]$Help,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs
)

$ErrorActionPreference = "Stop"
$scriptBoundParameters = $PSBoundParameters

& {
    $repo = "buYoung/mcp"
    $bin = "codemap-search"
    $exeName = "$bin.exe"

    $hasOptVersion = ($null -ne $scriptBoundParameters -and $scriptBoundParameters.ContainsKey("Version")) -or ([bool]$Version)
    $optVersion = $Version
    $hasOptInstallDir = ($null -ne $scriptBoundParameters -and $scriptBoundParameters.ContainsKey("InstallDir")) -or ([bool]$InstallDir)
    $optInstallDir = $InstallDir
    $optNoModifyPath = [bool]$NoModifyPath
    $optPrintTarget = [bool]$PrintTarget
    $optHelp = [bool]$Help

    if ($RemainingArgs) {
        for ($i = 0; $i -lt $RemainingArgs.Count; $i++) {
            $arg = $RemainingArgs[$i]
            switch -Wildcard -CaseSensitive ($arg) {
                "--version" {
                    if ($i + 1 -ge $RemainingArgs.Count -or -not $RemainingArgs[$i + 1]) {
                        throw "--version needs a version or tag (e.g. 0.14.2 or codemap-v0.14.2)"
                    }
                    $i++
                    $optVersion = $RemainingArgs[$i]
                    $hasOptVersion = $true
                }
                "--version=*" {
                    $optVersion = $arg.Substring(10)
                    if (-not $optVersion) {
                        throw "--version needs a version or tag (e.g. 0.14.2 or codemap-v0.14.2)"
                    }
                    $hasOptVersion = $true
                }
                "--install-dir" {
                    if ($i + 1 -ge $RemainingArgs.Count -or -not $RemainingArgs[$i + 1]) {
                        throw "--install-dir needs a directory path"
                    }
                    $i++
                    $optInstallDir = $RemainingArgs[$i]
                    $hasOptInstallDir = $true
                }
                "--install-dir=*" {
                    $optInstallDir = $arg.Substring(14)
                    if (-not $optInstallDir) {
                        throw "--install-dir needs a directory path"
                    }
                    $hasOptInstallDir = $true
                }
                "--no-modify-path" {
                    $optNoModifyPath = $true
                }
                "--print-target" {
                    $optPrintTarget = $true
                }
                "-h" {
                    $optHelp = $true
                }
                "--help" {
                    $optHelp = $true
                }
                default {
                    throw "unknown argument: $arg"
                }
            }
        }
    }

    if ($optHelp) {
        [Console]::Error.WriteLine("usage: install.ps1 [-Version <version>] [-InstallDir <dir>] [-NoModifyPath] [-PrintTarget] [-Help]")
        [Console]::Error.WriteLine("env: INSTALL_DIR, CODEMAP_VERSION, CODEMAP_NO_MODIFY_PATH")
        return
    }

    function Resolve-NormalizedVersion {
        param([string]$RawVersion)
        if (-not $RawVersion -or $RawVersion -eq "latest") {
            return ""
        }
        if ($RawVersion -match '\s' -or $RawVersion -match '/') {
            throw "invalid version: $RawVersion"
        }
        if ($RawVersion.StartsWith("codemap-v", [System.StringComparison]::Ordinal)) {
            if ($RawVersion.Length -le 9) {
                throw "invalid version: $RawVersion"
            }
            return $RawVersion
        }
        if ($RawVersion -match '^v[0-9]') {
            return "codemap-$RawVersion"
        }
        if ($RawVersion -match '^[0-9]') {
            return "codemap-v$RawVersion"
        }
        throw "invalid version: $RawVersion (expected e.g. 0.14.2, v0.14.2, codemap-v0.14.2, or latest)"
    }

    function Resolve-NormalizedDir {
        param([string]$DirPath)
        if (-not $DirPath) { return "" }
        $d = $DirPath.Trim()
        while ($d.Length -gt 1 -and ($d.EndsWith('\') -or $d.EndsWith('/'))) {
            if ($d -match '^[A-Za-z]:[\\/]$') {
                break
            }
            $d = $d.Substring(0, $d.Length - 1)
        }
        return $d
    }

    function Test-Truthy {
        param([string]$Value)
        if (-not $Value) { return $false }
        return $Value -match '^(?i)(1|true|yes)$'
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

    $rawVersion = if ($hasOptVersion) {
        $optVersion
    } elseif ($env:CODEMAP_VERSION) {
        $env:CODEMAP_VERSION
    } else {
        ""
    }
    $resolvedVersion = Resolve-NormalizedVersion -RawVersion $rawVersion
    $versionLabel = if ($resolvedVersion) { $resolvedVersion } else { "latest" }

    $arch = Resolve-Arch
    $target = switch ($arch) {
        "x86_64"  { "x86_64-pc-windows-msvc" }
        "aarch64" { "aarch64-pc-windows-msvc" }
    }
    $asset = "$bin-$target.zip"
    $shaAsset = "$bin-$target.zip.sha256"

    $base = if ($resolvedVersion) {
        "https://github.com/$repo/releases/download/$resolvedVersion"
    } else {
        "https://github.com/$repo/releases/latest/download"
    }
    $archiveUrl = "$base/$asset"
    $shaUrl = "$base/$shaAsset"

    $homeRoot = if ($HOME) { $HOME } elseif ($env:USERPROFILE) { $env:USERPROFILE } else { "" }
    $installDirRaw = if ($hasOptInstallDir) {
        $optInstallDir
    } elseif ($env:INSTALL_DIR) {
        $env:INSTALL_DIR
    } elseif ($homeRoot) {
        Join-Path $homeRoot ".local\bin"
    } else {
        throw "HOME is not set; pass -InstallDir or set INSTALL_DIR"
    }
    $resolvedInstallDir = Resolve-NormalizedDir -DirPath $installDirRaw
    if (-not $resolvedInstallDir) {
        throw "install directory must not be empty"
    }

    $skipModifyPath = $optNoModifyPath -or (Test-Truthy -Value $env:CODEMAP_NO_MODIFY_PATH)

    if ($optPrintTarget) {
        $osLabel = if ($env:CODEMAP_OS_OVERRIDE) { $env:CODEMAP_OS_OVERRIDE } else { "Windows_NT" }
        Write-Output "os:          $osLabel"
        Write-Output "arch:        $arch"
        Write-Output "target:      $target"
        Write-Output "version:     $versionLabel"
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

        [Console]::Error.WriteLine("Downloading $bin ($target, $versionLabel)...")
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
        [Console]::Error.WriteLine("Verifying sha256 checksum...")
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
        $resolvedInstallFull = Resolve-NormalizedDir -DirPath ((Get-Item -LiteralPath $resolvedInstallDir).FullName)

        $stageTmpDir = Join-Path $resolvedInstallFull (".$bin.tmp." + [System.Guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $stageTmpDir -Force | Out-Null
        $stagedBin = Join-Path $stageTmpDir $exeName
        $installedPath = Join-Path $resolvedInstallFull $exeName
        Copy-Item -LiteralPath $extractedBin -Destination $stagedBin -Force
        Move-Item -LiteralPath $stagedBin -Destination $installedPath -Force

        $installedLabel = $bin
        try {
            $verOut = & $installedPath --version 2>$null | Select-Object -First 1
            if ($LASTEXITCODE -eq 0 -and $verOut) {
                $trimmedVer = $verOut.ToString().Trim()
                if ($trimmedVer) {
                    $installedLabel = $trimmedVer
                }
            }
        } catch {
            $installedLabel = $bin
        }
        [Console]::Error.WriteLine("Installed $installedLabel to $installedPath")

        $normTarget = $resolvedInstallFull.TrimEnd('\', '/').ToLowerInvariant()
        $sessionEntries = @()
        if ($env:PATH) {
            $sessionEntries = ($env:PATH -split ';') | ForEach-Object { $_.Trim() } | Where-Object { $_ }
        }
        $inSessionPath = ($sessionEntries | ForEach-Object { $_.TrimEnd('\', '/').ToLowerInvariant() }) -contains $normTarget

        if ($skipModifyPath) {
            if (-not $inSessionPath) {
                [Console]::Error.WriteLine("")
                [Console]::Error.WriteLine("note: $resolvedInstallFull is not on your PATH.")
                [Console]::Error.WriteLine("      add it for this session:  `$env:PATH = `"$resolvedInstallFull;`$env:PATH`"")
            }
        } else {
            if (-not $inSessionPath) {
                $env:PATH = if ($env:PATH) { "$resolvedInstallFull;$env:PATH" } else { $resolvedInstallFull }
            }
            try {
                $userPathRaw = [Environment]::GetEnvironmentVariable("Path", "User")
                $userEntries = @()
                if ($userPathRaw) {
                    $userEntries = ($userPathRaw -split ';') | ForEach-Object { $_.Trim() } | Where-Object { $_ }
                }
                $inUserPath = ($userEntries | ForEach-Object { $_.TrimEnd('\', '/').ToLowerInvariant() }) -contains $normTarget
                if (-not $inUserPath) {
                    $newUserPath = if ($userEntries.Count -gt 0) {
                        ($userEntries + $resolvedInstallFull) -join ';'
                    } else {
                        $resolvedInstallFull
                    }
                    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
                    [Console]::Error.WriteLine("Added $resolvedInstallFull to User Path (open a new terminal to use $bin)")
                }
            } catch {
                [Console]::Error.WriteLine("")
                [Console]::Error.WriteLine("note: could not update User Path; add $resolvedInstallFull to PATH manually:")
                [Console]::Error.WriteLine("      `$env:PATH = `"$resolvedInstallFull;`$env:PATH`"")
            }
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

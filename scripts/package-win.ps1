<#
    Package the OpenDeck Ajazz AKP05 plugin for Windows.

    - Supports both GNU and MSVC toolchains
    - Ensures target exists via rustup
    - Builds the binary
    - Collects assets and manifest into a .sdPlugin folder
    - Copies the binary with the name from manifest.json (CodePathWin)
    - Produces build/opendeck-akp05.plugin.zip

    Usage:
      # GNU (requires MinGW-w64 on PATH; ideally default toolchain = stable-x86_64-pc-windows-gnu)
      powershell -ExecutionPolicy Bypass -File scripts/package-win.ps1 -Toolchain gnu

      # MSVC (requires Visual Studio Build Tools with C++ workload / link.exe)
      powershell -ExecutionPolicy Bypass -File scripts/package-win.ps1 -Toolchain msvc
#>

param(
    [ValidateSet('gnu','msvc')]
    [string]$Toolchain = 'gnu'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Ensure cargo is in PATH
$cargoPath = "$env:USERPROFILE\.cargo\bin"
if (Test-Path $cargoPath) {
    $env:PATH = "$cargoPath;$env:PATH"
}

function Invoke-Step {
    param(
        [Parameter(Mandatory=$true)] [string] $Message,
        [Parameter(Mandatory=$true)] [scriptblock] $Action
    )
    Write-Host "[+] $Message" -ForegroundColor Cyan
    & $Action
}

try {
    $scriptDir = $PSScriptRoot
    $repoRoot = Split-Path -Parent $scriptDir
    Set-Location $repoRoot

    if (-not (Test-Path 'manifest.json')) {
        throw 'manifest.json not found. Please run this script from the repository (it is designed to be called from scripts/).'
    }

    $manifest = Get-Content 'manifest.json' | ConvertFrom-Json
    $pluginUuid = $manifest.PluginUUID
    if (-not $pluginUuid) { throw 'PluginUUID missing in manifest.json' }
    $codePathWin = $manifest.CodePathWin
    if (-not $codePathWin) { throw 'CodePathWin missing in manifest.json' }

    $target = if ($Toolchain -eq 'msvc') { 'x86_64-pc-windows-msvc' } else { 'x86_64-pc-windows-gnu' }
    $toolchainName = if ($Toolchain -eq 'msvc') { 'stable-x86_64-pc-windows-msvc' } else { 'stable-x86_64-pc-windows-gnu' }
    $targetDir = Join-Path $repoRoot "target\plugin-win-$Toolchain"
    $buildExe = Join-Path $targetDir "$target\release\opendeck-akp05.exe"

    $buildDir = Join-Path $repoRoot 'build'
    $pluginFolderName = "$pluginUuid.sdPlugin"
    $pluginFolder = Join-Path $buildDir $pluginFolderName
    $zipPath = Join-Path $buildDir 'opendeck-akp05.plugin.zip'

    $hasRustup = (Get-Command rustup -ErrorAction SilentlyContinue) -ne $null
    if ($hasRustup) {
        Invoke-Step -Message "Ensuring Rust toolchain '$toolchainName'" -Action { & rustup toolchain install $toolchainName | Out-Null }
        Invoke-Step -Message "Ensuring Rust target '$target'" -Action { & rustup target add $target | Out-Null }
    } else {
        Write-Warning "'rustup' not found on PATH. Skipping toolchain/target installation."
        Write-Warning "Install rustup (winget install Rustlang.Rustup / choco install rustup.install) or ensure the requested toolchain/target is already available."
    }

    if ($Toolchain -eq 'gnu') {
        # Pre-flight: basic MinGW tools check
        $haveGcc = (Get-Command gcc -ErrorAction SilentlyContinue) -ne $null
        $haveDlltool = (Get-Command dlltool -ErrorAction SilentlyContinue) -ne $null
        if (-not $haveGcc -or -not $haveDlltool) {
            Write-Warning 'GNU build requires MinGW-w64 tools on PATH (gcc.exe, dlltool.exe).'
            Write-Warning 'Install MSYS2 and add C:\msys64\mingw64\bin to PATH, or install mingw-w64 via Chocolatey.'
            Write-Warning 'Alternatively, use -Toolchain msvc after installing Visual Studio Build Tools (C++).' 
        }
        Write-Host 'Note: If you see "link.exe not found" while using -Toolchain gnu, set default toolchain to GNU:' -ForegroundColor Yellow
        Write-Host '  rustup toolchain install stable-x86_64-pc-windows-gnu' -ForegroundColor Yellow
        Write-Host '  rustup default stable-x86_64-pc-windows-gnu' -ForegroundColor Yellow
    }

    if ($Toolchain -eq 'msvc') {
        # Pre-flight: MSVC linker check
        $haveLink = (Get-Command link.exe -ErrorAction SilentlyContinue) -ne $null
        if (-not $haveLink) {
            Write-Warning 'MSVC build requires Visual Studio Build Tools (C++ workload) providing link.exe.'
            Write-Warning 'Install VS 2019/2022 Build Tools with: Desktop development with C++ + Windows 10/11 SDK.'
            Write-Warning 'Attempting to locate and add Visual Studio tools to PATH...'

            # Try to find and add VS tools to PATH
            $vsPaths = @(
                "C:\Program Files\Microsoft Visual Studio\2022\Community",
                "C:\Program Files\Microsoft Visual Studio\2022\Professional",
                "C:\Program Files\Microsoft Visual Studio\2022\Enterprise",
                "C:\Program Files (x86)\Microsoft Visual Studio\2019\Community",
                "C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional"
            )

            foreach ($vsPath in $vsPaths) {
                if (Test-Path $vsPath) {
                    # Find the VC tools directory
                    $vcToolsBase = Join-Path $vsPath "VC\Tools\MSVC"
                    if (Test-Path $vcToolsBase) {
                        $vcVersion = Get-ChildItem $vcToolsBase | Sort-Object Name -Descending | Select-Object -First 1
                        if ($vcVersion) {
                            $vcToolsPath = Join-Path $vcToolsBase $vcVersion.Name
                            $vcBinPath = Join-Path $vcToolsPath "bin\Hostx64\x64"

                            # Find Windows SDK
                            $sdkBase = "C:\Program Files (x86)\Windows Kits\10"
                            if (Test-Path $sdkBase) {
                                $sdkLibBase = Join-Path $sdkBase "Lib"
                                $sdkVersion = Get-ChildItem $sdkLibBase | Where-Object { $_.Name -match '10\.0\.\d+\.\d+' } | Sort-Object Name -Descending | Select-Object -First 1

                                if ($sdkVersion) {
                                    Write-Host "Found Visual Studio at: $vsPath" -ForegroundColor Yellow
                                    Write-Host "Using VC Tools: $vcToolsPath" -ForegroundColor Yellow
                                    Write-Host "Using SDK: $($sdkVersion.Name)" -ForegroundColor Yellow

                                    $env:PATH = "$vcBinPath;$env:PATH"
                                    $env:LIB = "$vcToolsPath\lib\x64;$sdkBase\Lib\$($sdkVersion.Name)\um\x64;$sdkBase\Lib\$($sdkVersion.Name)\ucrt\x64"
                                    $env:INCLUDE = "$vcToolsPath\include;$sdkBase\Include\$($sdkVersion.Name)\ucrt;$sdkBase\Include\$($sdkVersion.Name)\um;$sdkBase\Include\$($sdkVersion.Name)\shared"
                                    break
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Invoke-Step -Message "Building Windows binary (release, $Toolchain)" -Action {
        if ($hasRustup) {
            & cargo "+$toolchainName" build --release --target $target --target-dir $targetDir
        } else {
            # Fall back to current default cargo toolchain
            & cargo build --release --target $target --target-dir $targetDir
        }
    }

    if (-not (Test-Path $buildExe)) {
        throw "Build output not found: $buildExe"
    }

    Invoke-Step -Message 'Preparing plugin folder' -Action {
        if (Test-Path $pluginFolder) { Remove-Item -Recurse -Force $pluginFolder }
        if (-not (Test-Path $buildDir)) { New-Item -ItemType Directory -Force -Path $buildDir | Out-Null }
        New-Item -ItemType Directory -Force -Path $pluginFolder | Out-Null
        Copy-Item -Recurse -Force 'assets' (Join-Path $pluginFolder 'assets')
        Copy-Item -Force 'manifest.json' (Join-Path $pluginFolder 'manifest.json')
        Copy-Item -Force $buildExe (Join-Path $pluginFolder $codePathWin)
    }

    Invoke-Step -Message 'Creating plugin zip' -Action {
        if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
        Compress-Archive -Path $pluginFolder -DestinationPath $zipPath -Force
    }

    Write-Host ''
    Write-Host "Done." -ForegroundColor Green
    Write-Host "Plugin folder: $pluginFolder"
    Write-Host "Plugin zip:    $zipPath"
    Write-Host ''
    Write-Host 'Install in OpenDeck: Plugins -> Install from file -> select the .zip file.'
}
catch {
    Write-Error $_
    exit 1
}

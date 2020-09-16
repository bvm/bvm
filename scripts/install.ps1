#!/usr/bin/env pwsh
# Adapted from Deno's install script (https://github.com/denoland/deno_install/blob/master/install.ps1)

$ErrorActionPreference = 'Stop'

if ($args.Length -gt 0) {
  $Version = $args.Get(0)
}

if ($PSVersionTable.PSEdition -ne 'Core') {
  $IsWindows = $true
  $IsMacOS = $false
}

$BinDir = if ($env:BVM_INSTALL_DIR) {
  "$env:BVM_INSTALL_DIR\bin"
} elseif ($IsWindows) {
  "$Home\.bvm\bin"
}

$BvmZip = "$BinDir\bvm.zip"

$BvmCmd = "$BinDir\bvm-bin.cmd"
$BvmExe = "$BinDir\bvm-bin.exe"

$Target = 'x86_64-pc-windows-msvc'

# GitHub requires TLS 1.2
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$BvmUri = if (!$Version) {
  "https://github.com/dsherret/bvm/releases/latest/download/bvm-${Target}.zip"
} else {
  "https://github.com/dsherret/bvm/releases/download/$Version/bvm-${Target}.zip"
}

if (!(Test-Path $BinDir)) {
  New-Item $BinDir -ItemType Directory | Out-Null
}

# stop any running bvm processes
Stop-Process -Name "bvm" -Erroraction 'silentlycontinue'

# download and install
Invoke-WebRequest $BvmUri -OutFile $BvmZip -UseBasicParsing

if (Get-Command Expand-Archive -ErrorAction SilentlyContinue) {
  Expand-Archive $BvmZip -Destination $BinDir -Force
} else {
  if (Test-Path $BvmExe) {
    Remove-Item $BvmExe
  }
  if (Test-Path $BvmCmd) {
    Remove-Item $BvmCmd
  }
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  [IO.Compression.ZipFile]::ExtractToDirectory($BvmZip, $BinDir)
}

Remove-Item $BvmZip

Start-Process -FilePath "$BinDir\bvm-bin" -ArgumentList "hidden-shell","windows-install","`"$BinDir`""

$Env:Path = "$env:APPDATA\bvm\shims;" + $Env:Path
$Env:Path = "$BinDir;" + $Env:Path

Write-Output "bvm was installed successfully to $BinDir"
Write-Output "Run 'bvm --help' to get started"

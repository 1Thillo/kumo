param(
  [string]$Exe = (Join-Path (Split-Path $PSScriptRoot) 'target\release\kumo.exe'),
  [string]$Out = (Join-Path $env:USERPROFILE 'Downloads\kumo-msix'),
  [string]$Name = 'HYOS.Kumo',
  [string]$Publisher = 'CN=HYOS',
  [string]$PublisherName = 'HYOS',
  [string]$Version = ''
)
# Builds the Microsoft Store package from a finished kumo.exe. For a submission, pass the values
# from "Product identity" in Partner Center. The Store signs the package itself.
# $Out must not be a folder with a Low integrity label, or the test install would run at Low.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
if (-not $Version) {
  $line = Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version = "(\d+)\.(\d+)\.(\d+)"' | Select-Object -First 1
  $Version = '{0}.{1}.{2}.0' -f $line.Matches[0].Groups[1].Value, $line.Matches[0].Groups[2].Value, $line.Matches[0].Groups[3].Value
}
if ($Version -notmatch '^[1-9]\d*\.\d+\.\d+\.0$') { throw "The Store needs a version like 1.0.0.0, not $Version" }

$kit = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Directory | Where-Object { Test-Path (Join-Path $_.FullName 'x64\makeappx.exe') } | Sort-Object Name | Select-Object -Last 1
if (-not $kit) { throw 'makeappx.exe not found: install the Windows SDK' }
$tools = Join-Path $kit.FullName 'x64'

$layout = Join-Path $Out 'layout'
Remove-Item -Recurse -Force $layout -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $layout | Out-Null
Copy-Item -Recurse (Join-Path $root 'package\Assets') (Join-Path $layout 'Assets')
[IO.File]::WriteAllBytes((Join-Path $layout 'kumo.exe'), [IO.File]::ReadAllBytes($Exe))
Copy-Item (Join-Path $root 'LICENSE') (Join-Path $layout 'LICENSE.txt')
$manifest = (Get-Content -Raw (Join-Path $root 'package\AppxManifest.xml')).Replace('@NAME@', $Name).Replace('@PUBLISHER_NAME@', $PublisherName).Replace('@PUBLISHER@', $Publisher).Replace('@VERSION@', $Version)
[IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest)

# resources.pri is what lets Windows pick the right logo size
$config = Join-Path $Out 'priconfig.xml'
& (Join-Path $tools 'makepri.exe') createconfig /cf $config /dq en-US_de-DE /o | Out-Null
& (Join-Path $tools 'makepri.exe') new /pr $layout /cf $config /of (Join-Path $layout 'resources.pri') /mn (Join-Path $layout 'AppxManifest.xml') /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'makepri failed' }

$msix = Join-Path $Out "Kumo-$Version.msix"
& (Join-Path $tools 'makeappx.exe') pack /d $layout /p $msix /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'makeappx failed' }
'{0}  {1:N1} MB' -f $msix, ((Get-Item $msix).Length / 1MB)

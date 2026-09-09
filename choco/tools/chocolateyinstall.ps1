$ErrorActionPreference = 'Stop'

# OpenKite Windows installer (NSIS) — arch-matched download from the
# GitHub release. The self-update job (release.yml → publish-choco)
# rewrites $version and both checksums before `choco pack`.
$version = '0.26.4'
$amd64Checksum = '0000000000000000000000000000000000000000000000000000000000000000'
$arm64Checksum = '0000000000000000000000000000000000000000000000000000000000000000'

$isArm64 = $env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64'
$arch = if ($isArm64) { 'arm64' } else { 'amd64' }
$checksum = if ($isArm64) { $arm64Checksum } else { $amd64Checksum }

$url = "https://github.com/jomakori/openkite/releases/download/v${version}/openkite_${version}_windows_${arch}.exe"

$packageArgs = @{
  packageName    = 'openkite'
  fileType       = 'exe'
  url            = $url
  checksum       = $checksum
  checksumType   = 'sha256'
  silentArgs     = '/S'            # NSIS silent install
  validExitCodes = @(0)
}

Install-ChocolateyPackage @packageArgs

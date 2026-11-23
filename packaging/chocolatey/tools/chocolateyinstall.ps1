$ErrorActionPreference = 'Stop'

$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

# Tokens below are substituted by the workflow before `choco pack`; nothing here
# reads an environment variable, so the package is reproducible from its nupkg.
$url      = 'URL_PLACEHOLDER'
$checksum = 'CHECKSUM_PLACEHOLDER'

$archive = Join-Path $toolsDir 'openkite.tar.gz'

# The release ships a tar.gz on every platform, Windows included, so there is no
# zip path here; Get-ChocolateyWebFile fetches it and checks the sha256 before
# anything is extracted.
Get-ChocolateyWebFile -PackageName 'openkite' `
  -FileFullPath $archive `
  -Url $url `
  -Checksum $checksum `
  -ChecksumType 'sha256'

# Windows runners carry tar.exe; no bsdtar/7zip dependency needed.
tar -xzf $archive -C $toolsDir
Remove-Item $archive

$binary = Get-ChildItem -Path $toolsDir -Filter 'openkite-web*' -File -Recurse | Select-Object -First 1
if (-not $binary) {
  throw 'openkite-web binary not found in the extracted archive.'
}

# Renaming to openkite.exe is the whole shim: Chocolatey auto-generates a PATH
# shim for every .exe left in the tools directory, named after the file.
Move-Item -Path $binary.FullName -Destination (Join-Path $toolsDir 'openkite.exe') -Force

if ($binary.DirectoryName -ne $toolsDir) {
  Remove-Item -Path $binary.DirectoryName -Recurse -Force
}

# openkite-web reads OPENKITE_VERSION at runtime before falling back to its build-time
# constant, so the shim's version comes from here rather than a rebuild per release.
Install-ChocolateyEnvironmentVariable -VariableName 'OPENKITE_VERSION' -VariableValue $env:ChocolateyPackageVersion -VariableType 'User'

# Same-job CI steps run as separate processes and do not pick up the registry change
# above until a new session; GITHUB_ENV lets the verification step see it immediately.
if ($env:GITHUB_ENV) {
  Add-Content -Path $env:GITHUB_ENV -Value "OPENKITE_VERSION=$env:ChocolateyPackageVersion"
}

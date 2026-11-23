# OpenKite Chocolatey package

Packages the `openkite-web` release binary as the `openkite` Chocolatey
package, with a shim so `openkite` lands on PATH.

## Feed

Published to the **GitHub Packages NuGet feed**
(`https://nuget.pkg.github.com/jomakori/index.json`) only. That feed takes a
GitHub token and no moderation, so every tagged release can publish
immediately. `community.chocolatey.org` needs its own API key and a human
review per version — not done by this package or its workflow.

## How it's built

`.github/workflows/chocolatey.yml` runs on `release: published` (and
`workflow_dispatch` for a manual run against the latest release). It:

1. Downloads the release's `windows-amd64` tar.gz asset and reads its sha256
   out of the release's `checksums.txt`.
2. Fills `openkite.nuspec`'s version and `tools/chocolateyinstall.ps1`'s
   download URL/checksum placeholders with those values.
3. `choco pack`s the nupkg and pushes it to the GitHub Packages feed.
4. In the same job, adds that feed as a Chocolatey source, `choco install`s
   the package it just pushed, and runs `openkite --version` to confirm the
   installed shim reports the tag version.

## Packing and installing locally (Windows + choco required)

```powershell
cd packaging/chocolatey

# Fill in the two placeholders chocolateyinstall.ps1 ships with: a release
# asset URL and its sha256 from that release's checksums.txt.
(Get-Content tools/chocolateyinstall.ps1) `
  -replace 'URL_PLACEHOLDER', 'https://github.com/jomakori/openkite/releases/download/v0.1.0/openkite_v0.1.0_windows_amd64.tar.gz' `
  -replace 'CHECKSUM_PLACEHOLDER', '<sha256 from that release''s checksums.txt>' |
  Set-Content tools/chocolateyinstall.ps1

(Get-Content openkite.nuspec) -replace 'VERSION_PLACEHOLDER', '0.1.0' |
  Set-Content openkite.nuspec

choco pack
choco install openkite -s . -y
openkite --version
```

## Shim

Chocolatey auto-shims every `.exe` left in the package's `tools` directory
under its own name. `chocolateyinstall.ps1` extracts the release tar.gz and
renames the `openkite-web(.exe)` binary it contains to `openkite.exe`, so the
generated shim is `openkite`, not `openkite-web`.

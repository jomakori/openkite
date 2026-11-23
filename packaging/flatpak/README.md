# Flatpak packaging

`io.github.jomakori.OpenKite.yml` builds `openkite-desktop` (the GTK +
WebKitGTK shell) from a release source tarball and installs the binary,
`.desktop` entry, AppStream metainfo and icon into `/app`.

## Sandbox

`finish-args` grants only what the desktop shell needs:

- `--share=ipc`, `--socket=fallback-x11`, `--socket=wayland`, `--device=dri` —
  display and GPU acceleration for the webview.
- `--share=network` — the engine talks to the cluster's API server.
- `--filesystem=~/.kube:ro` + `--env=KUBECONFIG=$HOME/.kube/config` — read the
  user's kubeconfig; nothing else on the host is visible to the app.

Runtime is `org.gnome.Platform` (not `org.freedesktop.Platform`) because it
carries WebKitGTK.

## Building locally

Requires `flatpak`, `flatpak-builder`, and the `org.gnome.Platform`/
`org.gnome.Sdk` runtime pair at the version pinned in the manifest
(`runtime-version`), plus the `org.freedesktop.Sdk.Extension.rust-stable`
extension:

```bash
flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpackagerepo
flatpak install --user flathub org.gnome.Platform//47 org.gnome.Sdk//47 \
  org.freedesktop.Sdk.Extension.rust-stable//47
```

The manifest's `sources` entry is a template: `url` and `sha256` are the
placeholders `@OPENKITE_SOURCE_URL@` / `@OPENKITE_SOURCE_SHA256@`, filled in
by `.github/workflows/flatpak.yml` from the tagged release's source tarball.
To build a specific tag locally, substitute both placeholders yourself first:

```bash
tag=v0.1.0
curl -fsSL -o src.tar.gz "https://github.com/jomakori/openkite/archive/refs/tags/${tag}.tar.gz"
sha256=$(sha256sum src.tar.gz | cut -d' ' -f1)
sed -e "s#@OPENKITE_SOURCE_URL@#https://github.com/jomakori/openkite/archive/refs/tags/${tag}.tar.gz#" \
    -e "s/@OPENKITE_SOURCE_SHA256@/${sha256}/" \
    io.github.jomakori.OpenKite.yml > rendered.yml

flatpak-builder --repo=repo --force-clean build-dir rendered.yml
flatpak build-bundle repo openkite.flatpak io.github.jomakori.OpenKite
flatpak install --user --noninteractive openkite.flatpak
flatpak run io.github.jomakori.OpenKite
```

## Flathub

This manifest targets a release-attached bundle only. Submitting it to the
Flathub repo (`flathub/flathub` PR, Flathub-side CI, review) is a separate,
later piece of work — not covered here.

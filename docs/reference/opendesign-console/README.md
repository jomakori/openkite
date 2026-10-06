# OpenDesign reference — vendored

The OpenKite console mockup, copied byte-for-byte from the OpenDesign project so
the design system can be graded **in-repo** and regenerated without network
access. `DESIGN.md` and `docs/design-surfaces.md` cite line numbers into the
`openkite-console.css` / `.html` copies in this directory.

**Do not edit these files.** Every line reference in `DESIGN.md` and
`docs/design-surfaces.md` is into these exact bytes; an edit silently
invalidates them. Update by re-grabbing (below) and re-deriving the docs.

## Source

| | |
|---|---|
| OpenDesign project | `0da67a18-9944-402b-81b1-90e47929f481` |
| Project name | OpenKite Kubernetes IDE Design |
| Applied plugin snapshot | `08768ac6-82e2-4938-a62e-7f5dfcc89bc9` |
| Bundled design system | *(none — `designSystemId: null`; the project defines its own tokens, which is why the token set is pinned in `DESIGN.md`)* |
| Grabbed | 2026-08-24 |
| API | `https://design.maklab.net` (in-cluster: `http://opendesign-open-design.opendesign.svc.cluster.local`) |

## Manifest

| File | Bytes | Kind | sha256 |
|---|---|---|---|
| `brand-spec.md` | 1830 | text | `b5d819701059c1e40d36d0a8017116fbe07c53cc771a3de00ab6f5a94054bb5f` |
| `openkite-console.css` | 24241 | code | `11aed3d3f58af7ed2eb6a340fb1399fd3520e2924905f8bdc85bdea01a6adbb1` |
| `openkite-console.html` | 38768 | html | `7f3e2931f56fad821f41b1e82116b9ef69c289cdbc2f5f24b03517015c9ed1e4` |
| `openkite-console.js` | 8045 | code | `607d0c5ce970eea371454c9266f6f46d2ae7216add0af49e75e777bf77f62554` |

These hashes were verified against the live source at grab time; re-check with
the loop in **Re-grab** below.

## What is where

- **`brand-spec.md`** — the reference's own token spec (the file the `design-md`
  skill generalises). The provenance of every value in `DESIGN.md`.
- **`openkite-console.css` `:1-30`** — the reference's `:root` token block. This
  is the canonical palette.
- **`openkite-console.css` `:31-969`** — the surface rules
  (`docs/design-surfaces.md` maps each surface to its line range).
- **`openkite-console.html`** — the artifact structure: `data-od-id` landmarks
  per surface, starting at `:56` (`.app`), `:129` (`.topbar`), `:156`
  (`#view-workloads`), `:287` (log panel), `:316` (`#view-argocd`), `:523`
  (mobile nav), `:541` (inspector), `:570` (toast).
- **`openkite-console.js`** — behavioural reference only (no tokens): inspector
  open/replace, toast, swipe, pull-to-refresh, dock tabs. Consult it for
  behaviour, never for a value.

## Re-grab

The grab script and the full API notes live in the `opendesign-mockup-grab`
skill. Fast path:

```bash
bash /opt/data/skills/media/opendesign-mockup-grab/scripts/opendesign-grab.sh \
  0da67a18-9944-402b-81b1-90e47929f481 --all
```

Or fetch a single file and re-verify the hash:

```bash
PID=0da67a18-9944-402b-81b1-90e47929f481
BASE=https://design.maklab.net
for f in brand-spec.md openkite-console.css openkite-console.html openkite-console.js; do
  printf '%-24s ' "$f"
  curl -s "$BASE/api/projects/$PID/files/$f" | sha256sum | cut -d' ' -f1
done
```

If a hash above changed, the reference moved: re-derive `DESIGN.md` and
`docs/design-surfaces.md` in the same PR, and say so in the PR body rather than
leaving the docs citing stale lines.

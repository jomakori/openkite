# OpenDesign reference — vendored

The OpenKite console mockup, copied byte-for-byte from the OpenDesign project so
the design system can be graded **in-repo** and regenerated without network
access. `DESIGN.md` and `docs/design-surfaces.md` cite line numbers into the
`openkite-console.css` / `.html` copies in this directory.

**Do not edit these files.** Every line reference in `DESIGN.md` and
`docs/design-surfaces.md` is into these exact bytes; an edit silently
invalidates them. Update by re-grabbing (below) and re-deriving the docs.

The mockup is the design source of truth: it is re-fetched from OpenDesign and
re-derived into the docs, never hand-edited to match what the app happens to
render.

## Source

| | |
|---|---|
| OpenDesign project | `0da67a18-9944-402b-81b1-90e47929f481` |
| Project name | OpenKite Kubernetes IDE Design |
| Applied plugin snapshot | `08768ac6-82e2-4938-a62e-7f5dfcc89bc9` |
| Bundled design system | *(none — `designSystemId: null`; the project defines its own tokens, which is why the token set is pinned in `DESIGN.md`)* |
| Fetched | 2026-11-09 |
| Files API | `/api/projects/0da67a18-9944-402b-81b1-90e47929f481/files/{filename}` on the OpenDesign host |
| API | `https://design.maklab.net` (in-cluster: `http://opendesign-open-design.opendesign.svc.cluster.local`) |

## Manifest

| File | Bytes | Live mtime (UTC) | Kind | sha256 |
|---|---|---|---|---|
| `brand-spec.md` | 1830 | 2026-08-15T09:09:05Z | text | `b5d819701059c1e40d36d0a8017116fbe07c53cc771a3de00ab6f5a94054bb5f` |
| `openkite-console.css` | 30368 | 2026-10-19T06:42:53Z | code | `cd9061b4b42fc17ed573cec0ea41af5992228edfcdd7f935568f67ed3158e04e` |
| `openkite-console.html` | 26614 | 2026-10-20T05:59:20Z | html | `612f8ca629a7365bd5642217d60c84e79b409e1587f18c78bb2e2d613c57aff4` |
| `openkite-console.js` | 305637 | 2026-10-20T05:59:20Z | code | `ef854e082cc577d7471a28b72579f40741e81ef08058cf108aba1019968e3c9b` |
| `openkite-charts.css` | 96746 | 2026-10-20T06:05:34Z | code | `0eaed301b552da72c7c21aea40c3323aeb29b59e071d3af43538a9c5a1c7a402` |
| `openkite-charts.js` | 10799 | 2026-10-19T16:45:30Z | code | `da9e478f464c9600c3fce18add45bc34e92fb2b5ef7244ed27bdd8fbb3fffe75` |
| `openkite-workloads.css` | 24615 | 2026-10-19T07:10:21Z | code | `4ec7385912cc00e5edbe639e693093872e276564950afd187ac7c833becb29fc` |

These hashes were verified against the live source at fetch time; re-check with
the loop in **Re-grab** below.

The `openkite-console.css` and `openkite-console.html` rows are enforced, not
just recorded: `crates/openkite-ui/tests/design.rs` pins both files and derives
its class/token contracts from them, so a re-grab fails CI until the contract
block in `docs/design-surfaces.md` (§ Reference contract) is re-derived. This
table is the provenance record; that block is the machine-checked one.

## What is where

- **`brand-spec.md`** — the reference's own token spec (the file the `design-md`
  skill generalises). The provenance of every value in `DESIGN.md`.
- **`openkite-console.css` `:1-29`** — the reference's `:root` token block. This
  is the canonical palette.
- **`openkite-console.css` `:30-1024`** — the shell surface rules
  (`docs/design-surfaces.md` maps each surface to its line range).
- **`openkite-console.html`** — the artifact structure: `data-od-id` landmarks
  per surface, starting at `:79` (`.app`), `:118` (`.topbar`), `:148`
  (`#view-overview`), `:163` (`#view-workloads`), `:215` (`#view-argocd`),
  `:232` (mobile nav), `:250` (dock), `:268` (inspector), `:303` (search sheet),
  `:317` (toast).
- **`openkite-console.js`** — behavioural reference only (no tokens): nav
  catalogue, inspector open/replace, toast, swipe, pull-to-refresh, dock tabs
  and split panes, menus and the search sheet. Consult it for behaviour, never
  for a value.
- **`openkite-charts.css`** / **`openkite-charts.js`** — the chart, sparkline,
  gauge, meter and trend system, the cluster and account popovers, and the
  pull-down search sheet.
- **`openkite-workloads.css`** — the menu popup, row/column/namespace menus, the
  bulk-select bar, the inspector refinements, dialogs, and the dock-v2 tabs and
  split panes.

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
for f in brand-spec.md \
         openkite-console.css openkite-console.html openkite-console.js \
         openkite-charts.css openkite-charts.js openkite-workloads.css; do
  printf '%-24s ' "$f"
  curl -s "$BASE/api/projects/$PID/files/$f" | sha256sum | cut -d' ' -f1
done
```

If a hash above changed, the reference moved: re-derive `DESIGN.md` and
`docs/design-surfaces.md` in the same PR, and say so in the PR body rather than
leaving the docs citing stale lines.

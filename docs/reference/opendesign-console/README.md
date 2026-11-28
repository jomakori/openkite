# OpenDesign reference — vendored

<<<<<<< HEAD
The OpenKite console mockup, copied from the OpenDesign project so the design
system can be graded **in-repo** and regenerated without network access.
`DESIGN.md` and `docs/design-surfaces.md` cite line numbers into the
`openkite-console.css` / `.html` copies in this directory.

**These three `.css` sheets are the edit surface for the console stylesheet.**
`crates/openkite-ui/assets/console.css` is generated from them by
`crates/openkite-ui/assets/build_console_css.py`; editing the generated file
instead is drift, and the next run deletes your change — that is how the glass
layer, the settings pane and the ambient field were nearly lost. Change a rule
here, then run the generator and commit both.

`openkite-console.html` and the `.js` files are still read-only references: they
were fetched from OpenDesign and are never hand-edited.

The sheets keep the mockup's values as their base and have since been extended
with the console's own rules (the fill/radius rungs, the Liquid Glass layer, the
settings pane, the ambient field). Where they differ from a fresh grab, the
in-repo rule is the shipped one; re-grabbing means re-applying those edits.
=======
The OpenKite console mockup, copied byte-for-byte from the OpenDesign project so
the design system can be graded **in-repo** and regenerated without network
access. `DESIGN.md` cites line numbers into the `openkite-console.css` / `.html`
copies in this directory.

**Treat these files as read-only.** Every line reference in `DESIGN.md` is into
these bytes; an edit silently invalidates them. Update by re-grabbing (below) and
re-deriving the docs.

The mockup is the design source of truth: it is re-fetched from OpenDesign and
re-derived into the docs, never hand-edited to match what the app happens to
render — and `openkite-console.css` is currently the exception that proves the
rule, see **Drift** below.
>>>>>>> 9791f5b (docs: retire the rebuild-in-progress claims and the dead pointers (OKT-274))

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
| `openkite-console.css` | 43673 | unknown — edited in-repo since the grab, see note below | code | `58c943efa1eba9a582e6f83f9f6ec27e4502abec6361c61991892ed81cc94642` |
| `openkite-console.html` | 26614 | 2026-10-20T05:59:20Z | html | `612f8ca629a7365bd5642217d60c84e79b409e1587f18c78bb2e2d613c57aff4` |
| `openkite-console.js` | 305637 | 2026-10-20T05:59:20Z | code | `ef854e082cc577d7471a28b72579f40741e81ef08058cf108aba1019968e3c9b` |
| `openkite-charts.css` | 96746 | 2026-10-20T06:05:34Z | code | `0eaed301b552da72c7c21aea40c3323aeb29b59e071d3af43538a9c5a1c7a402` |
| `openkite-charts.js` | 10799 | 2026-10-19T16:45:30Z | code | `da9e478f464c9600c3fce18add45bc34e92fb2b5ef7244ed27bdd8fbb3fffe75` |
| `openkite-workloads.css` | 24615 | 2026-10-19T07:10:21Z | code | `4ec7385912cc00e5edbe639e693093872e276564950afd187ac7c833becb29fc` |

These hashes are the **fetch-time** record; the loop in **Re-grab** re-checks them
against the live source. The three `.css` sheets have been extended in-repo since
the grab, so their bytes no longer match the rows above — expected, and the
manifest stays as provenance. Current bytes, which the generator reproduces:

| File (current) | Bytes | sha256 |
|---|---|---|
| `openkite-console.css` | 33569 | `48627f3a5de32bc1d9e0ffdbd6499c831a0af8c688ea15afcbc51869cc72385b` |
| `openkite-workloads.css` | 23034 | `b9185c7d666b18d76af3f2e127118c2ee7fa1ea7df0a592bd194655399ef2123` |
| `openkite-charts.css` | 121349 | `97189952058580f99f1544de7a4ba92f827f28ce422a33fb3e018be24fef5ba0` |

Nothing pins the fetch-time hashes; what is enforced is that
`crates/openkite-ui/assets/console.css` reproduces from these sheets —
`crates/openkite-ui/tests/console_css.rs` runs `build_console_css.py --check` in
the workspace test job and fails on any difference.

The sheets were edited in-repo through the design wave — the ambient gradient per
appearance (#321), the no-`backdrop-filter` scrim (#322), the grouped appearance
chip (#326), the usage-gauge drill-down (#329) — and #327 put them behind the
generator, so those edits are the generator's input instead of hand-maintained
copies that the next regeneration would delete.

## What is where

- **`brand-spec.md`** — the reference's own token spec (the file the `design-md`
  skill generalises). The provenance of every value in `DESIGN.md`.
- **`openkite-console.css` `:1-42`** — the `:root` token block, including the
  console's own `--fill`, radius and `--glass-*` rungs. This is the canonical
  palette.
- **`openkite-console.css` `:43-1088`** — the shell surface rules
  (`docs/design-surfaces.md` maps each surface to its selectors).
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
  pull-down search sheet. The sheet's tail also carries rules that are not
  charts: the settings modal, the appearance picker and the per-palette ambient
  field, because those blocks were appended to the generated stylesheet over
  several PRs and the sheet mirrors it byte for byte. Splitting them out is a
  stylesheet change, not a drift fix.
- **`openkite-workloads.css`** — the menu popup, row/column/namespace menus, the
  bulk-select bar, the inspector refinements, dialogs, and the dock-v2 tabs and
  split panes.

## Regenerate

Edit a rule in one of the three `.css` sheets, then:

```bash
python3 crates/openkite-ui/assets/build_console_css.py           # write console.css
python3 crates/openkite-ui/assets/build_console_css.py --check   # verify, exit 1 on drift
```

Commit the sheet and the generated file together. `--check` also refuses (exit
2) when the result would drop a custom property the committed stylesheet
already defines — the loss that shipped when the glass layer lived only in the
generated file; pass `--allow-drop <token>` when the removal is intended.

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
leaving the docs citing stale lines. If `openkite-console.css` is re-grabbed, run
the class diff in § Drift first — the in-repo copy carries changes the project does
not have yet, and a blind overwrite drops them.

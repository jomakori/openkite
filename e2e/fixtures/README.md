# Fixture plugins

Bundles in `plugins/` are real plugins, kept small enough that CI can load them
and grade what the host paints. They are not shipped, not installed by any setup
step, and every one of them must stay free of cluster calls.

## `parity-fixture`

The G3 cross-host parity gate needs a route that both hosts can render with no
cluster behind them. This bundle is that route:

| Contribution | Value |
|---|---|
| sidebar entry | `Parity Fixture` → `/plugins/parity-fixture` (`icon: grid`) |
| route | `/plugins/parity-fixture`, title `Parity Fixture` |
| stylesheet | `style.css`, semantic tokens only (`--fg`, `--surface`, `--border`, `--muted`) |
| cluster access | none — no `openkite.api.*`, no timers, no host detection |

Both registrations go through the documented bridge surface
(`docs/plugin-api.md`, "Registration API"): `main.js` is evaluated with
`window.openkite` already bootstrapped and `window.__openkite_plugin` set to the
bundle's name, and registers exactly one sidebar item and one route. The shell
titles a plugin route with the sidebar entry that points at it, so the label is
the only text either host needs to paint — which is what makes the markup
deterministic.

The bundle registers no renderer because it cannot register one today: the
bootstrap exposes `openkite.registerRouteRenderer(path)`, but the bridge has no
`route_renderer` arm (`crates/openkite-host/src/bridge.rs`, its `other =>`
catch-all) and rejects the registration, so no host ever mounts a
`JsRouteMount` for the path. The route is therefore painted entirely by the
shared crate on both hosts, and `style.css` arrives scoped but matches nothing
until a host can mount a plugin-painted route.

`style.css` is scoped by the loader under `[data-plugin="parity-fixture"]`
(`crates/openkite-ui/src/plugin_css.rs`), so it cannot leak into the console
chrome that the gate grades.

## Installing

`install-plugin.sh` copies the bundle into a host's plugins root. The root is the
caller's business, because the two hosts disagree about what one is:

```bash
e2e/fixtures/install-plugin.sh "$HOME/.openkite/plugins" desktop
e2e/fixtures/install-plugin.sh "$OPENKITE_WEB_ROOT/plugins" web
```

The loader call sites behind those paths:

- **desktop** — `crates/openkite-desktop/src/plugin_js.rs:106`
  `plugins_dir()` returns `~/.openkite/plugins`, and
  `plugin_js.rs:170 discover_plugins(root)` → `plugin_js.rs:410 collect_bundles`
  scans it at boot; the bundle is live on the next launch.
- **web** — nothing to call. `crates/openkite-web` has no bundle loader:
  `host.rs:113` publishes `Capabilities::server_side()` (`plugins: false`) and no
  file under `crates/openkite-web/src` reads a plugins directory. Installing into
  the static root stages the bundle at `/plugins/parity-fixture/…` for a capture
  (or the loader that lands next) to fetch; the host does not evaluate it.

## What proves what

| Host | Proof | Where |
|---|---|---|
| desktop | loader boundary: `install-plugin.sh` stages the bundle, `collect_bundles` loads it, its scoped stylesheet arrives, and the bundle's real bridge registrations reduce to one sidebar item + one route | `crates/openkite-desktop/tests/parity_fixture.rs` |
| web | registration boundary: the installed bundle's registrations through `POST /openkite` paint the nav entry, and the route renders the declared-unsupported `plugin-route` panel | `crates/openkite-web/tests/parity_fixture.rs` |

Both tests run without a display, because neither one starts a webview or a
server: the bundle runs in `node` against the real `OPENKITE_BRIDGE_JS`
bootstrap, and the registrations it posts are applied by the same
`apply_register` the hosts use. Both also install the fixture through
`install-plugin.sh` first, so the script CI is expected to call is itself
covered rather than assumed.

```bash
cargo test -p openkite --no-default-features --test parity_fixture
cargo test -p openkite-web --test parity_fixture
```

## Adding this route to the gate

Not done here, and not because of a red gate: the fixture's entry is appended
after the core nav sections (`shell.rs`, `PluginNavSlot` — "never through the
core list"), the gate grades each contract node on its first match
(`parity-snapshot.js`, `matches[0]`), and match-count differences are review
notices rather than deltas (`compare.mjs`). Installing into the desktop host
alone would therefore leave the four graded routes green while proving nothing,
because the fixture's own route is in none of the three lists. The next change
has to land all of:

1. the route in the three lists that hold it — `crates/openkite-ui/src/parity.rs`
   (`ROUTES`), `e2e/visual/browser/capture.mjs`, `e2e/parity/run.sh`;
2. the fixture installed for both hosts in the `parity` job
   (`.github/workflows/build-test-staging.yml`), through this script;
3. either a bundle loader for the web host (`collect_bundles` already compiles
   there — `openkite-web` depends on the desktop crate with
   `default-features = false` — so this is wiring, not a new subsystem) or a
   host-conditional entry in the gate's `CONTRACT`
   (`crates/openkite-ui/assets/parity-snapshot.js`).

Until (3) lands, `/plugins/parity-fixture` renders the plugin's page on the
desktop host and the declared-unsupported panel on the web host — the two
snapshots cannot agree, and the gate would correctly reject them.

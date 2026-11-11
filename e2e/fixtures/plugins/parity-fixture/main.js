// G3 parity fixture: the smallest bundle a real plugin can be.
//
// One sidebar entry and one route, registered through the documented bridge
// surface (docs/plugin-api.md, "Registration API"). Nothing else: no cluster
// calls, no `openkite.api.*`, no host-specific branching, no timers — the only
// input is this file's own constants, so the markup both hosts paint for
// /plugins/parity-fixture is deterministic run to run.
//
// The sidebar label is the text the route head renders as its title (the shell
// titles a plugin route with the entry that points at it), which is why the two
// registrations share it — and it is the route's whole painted text on purpose.
//
// A bundle cannot own its own markup yet: the bootstrap offers
// `openkite.registerRouteRenderer(path)`, but the bridge has no `route_renderer`
// arm (`crates/openkite-host/src/bridge.rs`, its `other =>` catch-all), so the
// registration is rejected and no `JsRouteMount` is ever created. The fixture
// therefore contributes registrations only, and both hosts paint the route
// through the shared crate. `style.css` is scoped and carried by the loader
// today; its rules match once a host can mount a plugin-painted route.
(function () {
  var LABEL = "Parity Fixture";
  var ROUTE = "/plugins/parity-fixture";

  window.openkite.registerSidebar({ label: LABEL, icon: "grid", route: ROUTE });
  window.openkite.registerRoute({ path: ROUTE, title: LABEL });
})();

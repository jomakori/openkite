// Test-only JS plugin for the OKT-95 bridge-dispatch regression guard.
//
// A `register` POST walks the path a runtime-mirror regression breaks:
//   fetch("/openkite") -> dispatch_bridge_post (tokio worker)
//     -> Bridge::handle_post -> apply_register -> MIRROR_TX ping
//     -> the Dioxus-side refresh_registrations write.
//
// The guard asserts the returned JSON envelope AND that no panic reached
// app.log: the panic was contained, so an envelope/status check alone would
// not have caught it. This bundle never throws into the host's eval.
(function () {
  if (!window.openkite || typeof window.openkite.registerStatusItem !== "function") {
    // Missing bootstrap: the guard's missing-envelope assertion is the signal.
    return;
  }
  window.openkite.registerStatusItem({ label: "OKT95-GUARD", color: "green" });
  window.openkite.registerSidebar({
    label: "OKT95-GUARD",
    icon: "grid",
    route: "/okt95-guard",
  });
})();

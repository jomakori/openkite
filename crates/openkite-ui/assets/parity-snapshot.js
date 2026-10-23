// G3 cross-host parity snapshot.
//
// One extractor, evaluated on both hosts: the browser host through the CDP
// harness (`e2e/visual/browser/capture.mjs` with SNAPSHOT_DIR set) and the
// desktop host through its `OPENKITE_PARITY_SNAPSHOT` hook
// (`crates/openkite-desktop/src/parity.rs`). Both load this file, so the two
// sides of the comparison cannot drift apart in what they measure.
//
// What it grades (see `e2e/parity/README.md` for the rationale):
//   * `tokens`   — every custom property the stylesheet declares, resolved on
//                  the document element. This is the token contract both hosts
//                  must paint with.
//   * `nodes`    — the declared contract selectors: class chain, contract
//                  `data-*` attributes, and the computed values of the
//                  properties named here. Styles are graded as computed values,
//                  never as pixels.
//   * `chrome` / `view` — class-token inventories. Collected for review and
//                  printed by the comparator, but not gated: the hosts run on
//                  different data (the desktop boots disconnected, the browser
//                  host serves fixtures), so counts legitimately differ.
//
// The result is a JSON string: the last expression is what `document::eval`
// sends back on the desktop and what `Runtime.evaluate` returns in Chrome.
(function () {
  'use strict';

  var SCHEMA = 1;

  // The graded contract. Every selector must be one the shared crate paints on
  // both hosts; the property list is what "same token values" means per node.
  var CONTRACT = [
    { selector: '.app', properties: ['display', 'background-color', 'color', 'font-family', 'font-size', 'line-height'] },
    { selector: '.topbar', properties: ['display', 'height', 'background-color', 'border-bottom-color', 'padding-left', 'padding-right', 'position', 'z-index'] },
    { selector: '.brand-word', properties: ['color', 'font-family', 'font-size', 'font-weight', 'letter-spacing'] },
    { selector: '.nav-item', properties: ['display', 'color', 'font-size', 'font-weight', 'gap', 'padding-left', 'padding-right', 'border-radius'] },
    { selector: '.nav-item.active', properties: ['color', 'background-color', 'border-radius'] },
    { selector: '.nav-section .eyebrow', properties: ['color', 'font-size', 'font-weight', 'letter-spacing', 'text-transform'] },
    { selector: '.sidebar', properties: ['display', 'width', 'background-color', 'border-right-color'] },
    { selector: '.status-line', properties: ['color', 'font-size'] },
    { selector: '.view', properties: ['display', 'background-color', 'padding-top', 'padding-left'] },
    { selector: '.page-head', properties: ['display', 'align-items', 'gap', 'padding-top', 'padding-bottom', 'border-bottom-color'] },
    { selector: '.page-head h1', properties: ['color', 'font-size', 'font-weight', 'line-height', 'margin-top', 'margin-bottom'] },
    { selector: '.page-sub', properties: ['color', 'font-size', 'margin-top', 'margin-bottom'] },
    { selector: '.toolbar', properties: ['display', 'align-items', 'gap', 'padding-top', 'padding-bottom', 'background-color', 'border-bottom-color'] },
    // The routed page's own markers: `data-route` is how the gate proves both
    // hosts resolved the same route (the desktop webview has no URL history of
    // its own, so `location.pathname` is informational only).
    { selector: '[data-route]', properties: [] },
    { selector: '.tag-row', properties: ['display', 'gap', 'padding-top', 'border-top-color'] },
    { selector: '.search-field input', properties: ['background-color', 'border-top-color', 'border-radius', 'color', 'font-size', 'height', 'padding-left'] },
    { selector: '.tag', properties: ['background-color', 'border-radius', 'color', 'font-size', 'font-weight', 'padding-left', 'padding-right'] },
    { selector: '.btn', properties: ['background-color', 'border-radius', 'color', 'font-size', 'font-weight', 'padding-left', 'padding-right'] },
    { selector: '.panel', properties: ['background-color', 'border-radius', 'border-top-color', 'padding-left', 'padding-top'] }
  ];

  // `data-*` attributes the host owns (its capability verdict, its live state,
  // its transient interaction flags). Those are allowed to differ; the ones
  // left out of this list are contract markers ("this is route /workloads",
  // "this is the toolbar") and must match.
  var HOST_OWNED_ATTRS = [
    'data-state',
    'data-unsupported',
    'data-count',
    'data-empty',
    'data-error',
    'data-loading',
    'data-selected',
    'data-pinned',
    'data-open',
    'data-pressed',
    'data-hover',
    'data-width',
    'data-busy'
  ];

  // Region markers. `[data-surface]` other than the app root is host-supplied
  // content (the console lives only in the browser host since OKT-137);
  // `[data-state]` is a declared state stand-in, which G4 grades. Neither is
  // G3's subject, so both are skipped when the class inventories are taken.
  var HOST_SURFACE = '[data-surface]';
  var STATE_STAND_IN = '[data-state]';

  function collapse(value) {
    return String(value === null || value === undefined ? '' : value)
      .replace(/\s+/g, ' ')
      .replace(/^["'\s]+|["'\s]+$/g, '')
      .trim();
  }

  // Computed values that name fonts carry optional quotes that the engines
  // serialise differently; drop them so quoting cannot fail the gate.
  function normaliseFontFamily(value) {
    return collapse(value).replace(/["']/g, '');
  }

  function computedValue(computed, property) {
    var raw = computed.getPropertyValue(property);
    return property === 'font-family' ? normaliseFontFamily(raw) : collapse(raw);
  }

  function skipped(el, root) {
    if (el === root) return false;
    if (el.hasAttribute('data-surface')) return true;
    if (el.hasAttribute('data-state')) return true;
    return false;
  }

  function walk(root, visit) {
    var stack = [root];
    while (stack.length) {
      var el = stack.pop();
      if (skipped(el, root)) continue;
      visit(el);
      for (var i = el.children.length - 1; i >= 0; i--) stack.push(el.children[i]);
    }
  }

  function inventory(root) {
    var counts = {};
    walk(root, function (el) {
      for (var i = 0; i < el.classList.length; i++) {
        var name = el.classList[i];
        counts[name] = (counts[name] || 0) + 1;
      }
    });
    var ordered = {};
    Object.keys(counts).sort().forEach(function (name) {
      ordered[name] = counts[name];
    });
    return ordered;
  }

  function customPropertyNames() {
    var seen = {};
    function harvest(rules) {
      for (var i = 0; i < rules.length; i++) {
        var rule = rules[i];
        var style = rule.style;
        if (style) {
          for (var j = 0; j < style.length; j++) {
            var prop = style[j];
            if (prop.indexOf('--') === 0) seen[prop] = true;
          }
        }
        if (rule.cssRules) harvest(rule.cssRules);
      }
    }
    for (var s = 0; s < document.styleSheets.length; s++) {
      try {
        var rules = document.styleSheets[s].cssRules;
        if (rules) harvest(rules);
      } catch (error) {
        // A cross-origin sheet cannot be read; the hosts inline their CSS.
      }
    }
    var inline = document.documentElement.style;
    for (var k = 0; k < inline.length; k++) {
      var name = inline[k];
      if (name.indexOf('--') === 0) seen[name] = true;
    }
    return Object.keys(seen).sort();
  }

  function resolvedTokens() {
    var root = getComputedStyle(document.documentElement);
    var out = {};
    customPropertyNames().forEach(function (name) {
      out[name] = collapse(root.getPropertyValue(name));
    });
    return out;
  }

  function nodeReport(entry) {
    var selector = entry.selector;
    var matches = document.querySelectorAll(selector);
    if (!matches.length) return { present: false, count: 0 };
    var el = matches[0];
    var classes = [];
    for (var c = 0; c < el.classList.length; c++) classes.push(el.classList[c]);
    classes.sort();
    var attrs = {};
    for (var a = 0; a < el.attributes.length; a++) {
      var attr = el.attributes[a];
      if (attr.name.indexOf('data-') !== 0) continue;
      if (HOST_OWNED_ATTRS.indexOf(attr.name) !== -1) continue;
      attrs[attr.name] = collapse(attr.value);
    }
    var computed = getComputedStyle(el);
    var style = {};
    (entry ? entry.properties : []).forEach(function (property) {
      style[property] = computedValue(computed, property);
    });
    return { present: true, count: matches.length, classes: classes, attrs: attrs, style: style };
  }

  function band(width) {
    if (width <= 767) return 'narrow';
    if (width <= 1024) return 'medium';
    return 'wide';
  }

  function rounded(value) {
    var number = Number(value);
    return isNaN(number) ? 0 : Math.round(number * 1000) / 1000;
  }

  var errors = [];
  var app = document.querySelector('[data-surface="app"]');
  if (!app) errors.push('no [data-surface="app"] root: the shell did not mount');
  var view = app ? app.querySelector('.view') : null;
  if (app && !view) errors.push('no .view inside the app shell: the routed region did not mount');

  var nodes = {};
  CONTRACT.forEach(function (entry) {
    nodes[entry.selector] = nodeReport(entry);
  });

  var routeMarker = null;
  if (view) {
    var marker = view.querySelector('[data-route]');
    routeMarker = marker ? collapse(marker.getAttribute('data-route')) : null;
  }

  var snapshot = {
    schema: SCHEMA,
    route: location.pathname,
    route_marker: routeMarker,
    viewport: {
      width: rounded(window.innerWidth),
      height: rounded(window.innerHeight),
      dpr: rounded(window.devicePixelRatio || 1),
      band: band(window.innerWidth)
    },
    mounted: { app: !!app, view: !!view },
    tokens: resolvedTokens(),
    chrome: app ? inventory(app) : null,
    view: view ? inventory(view) : null,
    nodes: nodes,
    errors: errors
  };

  var json = JSON.stringify(snapshot);
  // Desktop transport: `document::eval` receives values the script sends.
  // Browser transport: `Runtime.evaluate` returns the expression's value.
  // Same string either way, so the two hosts cannot disagree on the payload.
  if (typeof dioxus !== 'undefined' && dioxus && typeof dioxus.send === 'function') {
    dioxus.send(json);
  }
  return json;
})()

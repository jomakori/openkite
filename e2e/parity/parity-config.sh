#!/usr/bin/env bash
# Configuration for the console parity gate (OKT-129) — sourced by compare.sh.
#
# Kept shell-simple on purpose: this repo has no jq in the gate path, and every
# number here has to be readable in a diff. Anything that is excluded from the
# comparison must say WHICH capability explains it; an unexplained difference
# fails the gate rather than being absorbed into a tolerance.

# --- frame ---------------------------------------------------------------
# Both captures are the full frame at this size: the browser viewport, and the
# desktop client area with the menu bar hidden (`menuBar = "hide"`, isolated
# $HOME). Same number on both sides is what makes this a 1:1 comparison with no
# crop constant — capture-desktop.sh asserts its frames against it, and
# compare.sh asserts every image it reads.
FRAME="800x600"

# --- tolerance -----------------------------------------------------------
# AE = ImageMagick's absolute count of pixels that differ beyond the per-channel
# fuzz. The two sides are different renderers (Chromium/Skia vs
# WebKitGTK/Cairo), so text antialiasing and glyph metrics differ slightly even
# with identical fonts: `-fuzz` absorbs that, the pixel cap bounds the rest.
#
# PARITY-CALIBRATION (measured, not guessed):
#   - browser-vs-baseline (same renderer, same job): measured in a clean run;
#     BROWSER_SELF_MAX is 2x the largest observed value, rounded up. A browser
#     self-diff above this means the gate is measuring noise, not a regression.
#   - desktop-vs-baseline (cross-renderer): MAX_DIFF_PIXELS is 2x the largest
#     observed value over three repeat captures per side, rounded up.
#   - the browser-vs-desktop column is reported for diagnosis at the same cap:
#     if each side is inside its own cap but the two frames disagree by more
#     than MAX_DIFF_PIXELS, the tolerances themselves need revisiting.
FUZZ="8%"
# 1% of 480,000 px (800x600). See PARITY-CALIBRATION above before changing it:
# a threshold that moves without a recorded measurement is a silenced gate.
MAX_DIFF_PIXELS=4800
# 0.1% of 480,000 px.
BROWSER_SELF_MAX=480

# --- masks ---------------------------------------------------------------
# Rectangles (`x,y,w,h`) painted out on BOTH sides before comparing, each with
# the capability that justifies it. Add one only with a recorded measurement in
# the reason — never to silence a real diff. Total masked area is capped below.
MASK_RECTS=()
MAX_MASKED_FRACTION=2 # % of the frame the masks may cover, in total

# --- surfaces ------------------------------------------------------------
# The console-owned routes the desktop can be booted onto deterministically via
# OPENKITE_ROUTE: /workloads (pods), /cluster (overview), /config (configmaps).
SURFACES=(01-pods 02-overview 03-configmaps)

# --- capability-attributable exclusions ----------------------------------
# Surfaces/features that are NEVER compared, because the browser target has no
# equivalent to render. Format: "<id>:<what capability explains it>".
EXCLUDED=(
  "terminal:desktop-only PTY view (src/router.rs console_route → None); the console has no terminal surface"
  "logs:desktop-only native Dioxus view; not a console surface"
  "plugin-views:desktop-only plugin directory and plugin-rendered DOM (e.g. the Argo CD plugin view)"
  "menu-bar-and-window-chrome:desktop-only platform chrome, removed from the frame by hiding the menu bar and capturing the client area; still covered by the desktop baselines in e2e/baselines/"
)

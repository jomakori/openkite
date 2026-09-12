#!/usr/bin/env bash
# OpenKite visual-capture payload (runs inside the capture Job, see
# capture-job.yaml.tmpl). It builds the app from a branch, boots it under
# Xvfb + openbox, and produces the documentation media:
#
#   console-shell.png       full-window still of the React console (route /)
#   console-workloads.png   full-window still of the live resource table
#   native-logs-route.png   still of the native shell on /logs (not ported)
#   resource-table.gif      live rows as scratch pods appear and disappear
#   inspector.gif           row -> inspector slide-over, then scrim-close
#   toast.gif               refresh -> toast acknowledgement -> auto-dismiss
#   log-dock.gif            log dock pause / collapse / restore
#   table-controls.gif      sort header + compact-density toggles
#   command-palette.gif     native palette opens over the console
#   native-menu-bar-{shown,hidden}.png   OS menu-bar visibility toggle
#   titlebar-{system,light,dark}.png     OS decoration theme override
#
# All media lands on the cargo-cache volume under MEDIA_DIR; the wrapper
# (capture.sh) fetches it with `kubectl cp`. The job log carries byte sizes and
# frame-diff evidence (max consecutive absolute-error pixels, max_ae) that
# proves the GIFs move.
#
# Why this runs in-cluster: a Dioxus link needs >2.5 GiB and OOMs a small host
# container. Never build the openkite binary on the review machine.
#
# Input automation notes (each cost real time to learn):
#   - Keyboard paging into the React webview is unreliable: nothing on the
#     React table is bound to Page_Down/Down/Up, and those never reached the
#     webview in the earlier harness. Drive the console with coordinate clicks
#     at the deterministic layout, and animate data from the cluster side.
#   - The X root is the reliable surface to photograph; each shot is cropped to
#     the app window's client geometry (recorded per boot).
#   - The palette IS reachable with ctrl+p after a click focuses the webview;
#     that is the one keyboard path we rely on, matching e2e/run-desktop-e2e.sh.
set -uxo pipefail

ART=/work/art
MEDIA="${MEDIA_DIR:-/data/media}"
BRANCH="${BRANCH:-main}"
SETTLE_S="${SETTLE_S:-50}"
SAMPLE_PODS="${SAMPLE_PODS:-4}"
SCRATCH_NS="${SCRATCH_NS:-}"
SHELL_ROUTE="${SHELL_ROUTE:-/}"
NATIVE_ROUTE="${NATIVE_ROUTE:-/logs}"
CAPTURE_FLOWS="${CAPTURE_FLOWS:-1}"
EXTRA_ROUTES="${EXTRA_ROUTES:-}"
WIN_W="${WIN_W:-1440}"
WIN_H="${WIN_H:-900}"

SAMPLE_PREFIX="aaa-ok-capture"
SAMPLE_LABEL="ok-capture=sample"
mkdir -p "$ART" "$MEDIA"

# --- deterministic React-console coordinates (window-relative px) ----------
# Layout: 252px sidebar, 56px sticky topbar, .view padding 20. See
# web/src/theme.css.
ROW1_X=700;   ROW1_Y=305     # first resource-table row (click -> inspector)
SCRIM_X=120;  SCRIM_Y=450    # inspector scrim (outside the 420px panel)
REFRESH_X=1294; REFRESH_Y=28 # topbar "Refresh resources" (raises a toast)
FOCUS_X=800;  FOCUS_Y=28     # empty topbar area (focuses the webview)
SORT_NAME_X=300; SORT_Y=263  # "Name" sortable table header
COMPACT_X=1369; TOOLBAR_Y=208 # right-most "Compact" density chip
PAGER_NEXT_X=1382; PAGER_Y=638 # pager "Next" button
LOG_Y=704                     # log-dock header icon row
LOG_PAUSE_X=1284; LOG_COLLAPSE_X=1334; LOG_CLEAR_X=1384

strip() { sed -e 's/\x1b\[[0-9;]*m//g'; }

# Largest absolute-error (differing pixel count) between consecutive frames.
# This is the motion metric reported in the job log: >0 means the GIF moves.
maxdiff() {
  local prev="" max=0 d
  for f in "$@"; do
    [ -f "$f" ] || continue
    if [ -n "$prev" ]; then
      d=$(compare -metric AE "$prev" "$f" null: 2>&1 || true)
      d="${d%%.*}"
      [ -n "$d" ] && [ "$d" -gt "$max" ] 2>/dev/null && max="$d"
    fi
    prev="$f"
  done
  echo "$max"
}

cleanup_samples() {
  [ -n "${SCRATCH_NS:-}" ] || return 0
  kubectl delete pod -n "$SCRATCH_NS" -l "$SAMPLE_LABEL" \
    --ignore-not-found --wait=false >/dev/null 2>&1 || true
}
trap cleanup_samples EXIT

echo "=== 1. system deps ==="
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev \
  libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev \
  pkg-config build-essential cmake \
  xvfb xauth dbus-x11 openbox xdotool imagemagick \
  git ca-certificates curl
KUBECTL_VER="$(curl -fsSL https://dl.k8s.io/release/stable.txt)"
curl -fsSLo /usr/local/bin/kubectl \
  "https://dl.k8s.io/release/${KUBECTL_VER}/bin/linux/amd64/kubectl"
chmod +x /usr/local/bin/kubectl
echo "kubectl=$(kubectl version --client=true 2>/dev/null | head -1)"

echo "=== 2. clone + build ==="
[ -d /src/.git ] || git clone --depth 50 --branch "$BRANCH" \
  https://github.com/jomakori/openkite.git /src
cd /src
git fetch -q origin "$BRANCH" 2>/dev/null || true
git checkout -q "$BRANCH" 2>/dev/null || true
git reset --hard -q "origin/$BRANCH" 2>/dev/null || true
echo "branch=$(git rev-parse --abbrev-ref HEAD)@$(git rev-parse --short HEAD)"
cargo build --bin openkite -j2 || { echo BUILD_FAILED; exit 1; }
BIN="$CARGO_TARGET_DIR/debug/openkite"
[ -x "$BIN" ] || { echo "BUILD_FAILED: no binary"; exit 1; }

echo "=== 3. display ==="
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
       LIBGL_ALWAYS_SOFTWARE=1 NO_COLOR=1
Xvfb :99 -screen 0 "$((WIN_W + 160))x$((WIN_H + 120))x24" >"$ART/xvfb.log" 2>&1 &
sleep 3
export DISPLAY=:99
openbox >"$ART/wm.log" 2>&1 &
sleep 2

WID=""; APID=""; X=0; Y=0; WIDTH=0; HEIGHT=0

# The host titles its window "OpenKite"; fall back to the WM class.
find_win() {
  xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | tail -1
}
find_win_class() {
  xdotool search --onlyvisible --class "openkite" 2>/dev/null | tail -1
}

geom() { eval "$(xdotool getwindowgeometry --shell "$WID" 2>/dev/null)" 2>/dev/null || true; }

# Capture the composited X root, then crop to the app window's client area.
# (The webview does not always photograph by window id; the root does.)
shoot() {
  import -window root "$ART/.raw.png" 2>/dev/null || return 0
  if [ -n "${WID:-}" ]; then
    geom
    if [ -n "${WIDTH:-}" ] && [ "${WIDTH:-0}" -gt 0 ]; then
      convert "$ART/.raw.png" -crop "${WIDTH}x${HEIGHT}+${X}+${Y}" +repage "$1" \
        2>/dev/null && return 0
    fi
  fi
  cp "$ART/.raw.png" "$1"
}

click_at() { # $1,$2 = window-relative x,y
  geom
  xdotool mousemove --sync "$((X + $1))" "$((Y + $2))" click 1
  sleep 0.15
}
park_mouse() { xdotool mousemove --sync 1 1; }
press_key() { xdotool key --clearmodifiers "$1"; }

focus_webview() {
  park_mouse
  click_at "$FOCUS_X" "$FOCUS_Y"
  sleep 0.4
}

open_palette() {
  xdotool windowactivate --sync "$WID" 2>/dev/null || true
  xdotool windowfocus --sync "$WID" 2>/dev/null || true
  xdotool key --clearmodifiers ctrl+p
}

boot() { # $1 route, $2 slug
  OPENKITE_ROUTE="$1" stdbuf -oL -eL "$BIN" >"$ART/app-$2.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 90); do
    WID="$(find_win)"; [ -z "$WID" ] && WID="$(find_win_class)"
    [ -n "$WID" ] && break
    sleep 1
  done
  if [ -n "$WID" ]; then
    xdotool windowsize --sync "$WID" "$WIN_W" "$WIN_H" 2>/dev/null || true
    xdotool windowmove --sync "$WID" 0 0 2>/dev/null || true
    xdotool windowactivate --sync "$WID" 2>/dev/null || true
  fi
  for i in $(seq 1 "$SETTLE_S"); do
    if grep -aq "live: reflectors running" "$ART/app-$2.log" && [ "$i" -ge 12 ]; then
      echo "reflectors_running_after_s=$i"
      break
    fi
    sleep 1
  done
  # Wait for the first pods snapshot so the table is not empty mid-shot.
  for i in $(seq 1 "$SETTLE_S"); do
    if grep -aq "live: snapshot kind=pods" "$ART/app-$2.log"; then
      echo "pods_snapshot_after_s=$i"
      break
    fi
    sleep 1
  done
  sleep 6 # let the webview finish its first paint
  geom
  echo "route=$1 slug=$2 window=${WID:-NONE} geom=${WIDTH}x${HEIGHT}+${X}+${Y}" \
    "alive=$(kill -0 "$APID" 2>/dev/null && echo yes || echo no)"
}

stop_app() {
  [ -n "${APID:-}" ] || return 0
  kill "$APID" 2>/dev/null || true
  sleep 3
  APID=""
  WID=""
}

N=0; CUR=""
start_frames() { CUR="$1"; N=0; rm -f "$ART/f-$CUR-"*.png; }
snap() { N=$((N + 1)); shoot "$(printf '%s/f-%s-%02d.png' "$ART" "$CUR" "$N")"; }

build_gif() { # $1 out.gif, $2 delay_cs, rest implied via CUR glob
  local out="$1" delay="$2"
  convert -delay "$delay" -loop 0 "$ART"/f-$CUR-*.png \
    -resize 900x -colors 200 -layers Optimize "$out" \
    2>"$ART/convert-$(basename "$out").log" \
    || convert -delay "$delay" -loop 0 "$ART"/f-$CUR-*.png -resize 900x "$out" 2>/dev/null || true
  rm -f "$ART"/.gf-*.png
  convert "$out" -coalesce "$ART/.gf-%03d.png" 2>/dev/null || true
  echo "GIF $(basename "$out") bytes=$(stat -c%s "$out" 2>/dev/null || echo 0)" \
    "frames=$(ls "$ART"/.gf-*.png 2>/dev/null | wc -l)" \
    "max_ae=$(maxdiff "$ART"/.gf-*.png)"
}

rows_now() {
  grep -a 'snapshot kind=pods' "$ART/app-table.log" 2>/dev/null | strip \
    | tail -1 | grep -oE 'rows=[0-9]+' | head -1
}

echo "=== 4. console shell still (route $SHELL_ROUTE) ==="
boot "$SHELL_ROUTE" "shell"
park_mouse
shoot "$MEDIA/console-shell.png"
echo "console-shell.png bytes=$(stat -c%s "$MEDIA/console-shell.png" 2>/dev/null || echo 0)"
stop_app

echo "=== 5. live resource table (route /workloads) ==="
boot "/workloads" "table"
park_mouse
shoot "$MEDIA/console-workloads.png"
echo "console-workloads.png bytes=$(stat -c%s "$MEDIA/console-workloads.png" 2>/dev/null || echo 0)"

if [ "$CAPTURE_FLOWS" = "1" ]; then
  ROWS_BEFORE="$(rows_now)"
  start_frames table
  snap
  i=0
  while [ "$i" -lt "$SAMPLE_PODS" ]; do
    i=$((i + 1))
    cat >"$ART/sample.yaml" <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: ${SAMPLE_PREFIX}-${i}
  namespace: ${SCRATCH_NS}
  labels: { ok-capture: sample }
spec:
  restartPolicy: Never
  containers:
    - name: sample
      image: registry.k8s.io/pause:3.9
      imagePullPolicy: IfNotPresent
EOF
    kubectl apply -f "$ART/sample.yaml" >/dev/null 2>&1 \
      || echo "WARN could not create ${SAMPLE_PREFIX}-${i} in ${SCRATCH_NS}"
    sleep 3
    snap
  done
  sleep 2; snap
  cleanup_samples
  sleep 4; snap
  sleep 3; snap
  ROWS_AFTER="$(rows_now)"
  echo "table_rows_before=${ROWS_BEFORE:-none} after=${ROWS_AFTER:-none}"
  build_gif "$MEDIA/resource-table.gif" 80

  echo "=== 6. inspector slide-over GIF ==="
  park_mouse
  start_frames inspector
  snap
  click_at "$ROW1_X" "$ROW1_Y"; sleep 0.1; snap
  sleep 0.3; snap
  sleep 0.6; snap
  click_at "$SCRIM_X" "$SCRIM_Y"; sleep 0.1; snap
  sleep 0.5; snap
  build_gif "$MEDIA/inspector.gif" 45

  echo "=== 7. toast GIF ==="
  park_mouse
  start_frames toast
  snap
  click_at "$REFRESH_X" "$REFRESH_Y"; sleep 0.2; snap
  sleep 0.5; snap
  sleep 1.8; snap
  build_gif "$MEDIA/toast.gif" 70

  echo "=== 8. table controls GIF (sort + density) ==="
  park_mouse
  start_frames controls
  snap
  click_at "$SORT_NAME_X" "$SORT_Y"; sleep 0.3; snap
  click_at "$COMPACT_X" "$TOOLBAR_Y"; sleep 0.3; snap
  click_at "$COMPACT_X" "$TOOLBAR_Y"; sleep 0.3; snap
  click_at "$PAGER_NEXT_X" "$PAGER_Y"; sleep 0.3; snap
  build_gif "$MEDIA/table-controls.gif" 60

  echo "=== 9. log dock GIF (pause + collapse) ==="
  park_mouse
  start_frames logdock
  snap
  click_at "$LOG_PAUSE_X" "$LOG_Y"; sleep 0.3; snap
  click_at "$LOG_COLLAPSE_X" "$LOG_Y"; sleep 0.3; snap
  click_at "$LOG_COLLAPSE_X" "$LOG_Y"; sleep 0.3; snap
  click_at "$LOG_CLEAR_X" "$LOG_Y"; sleep 0.3; snap
  build_gif "$MEDIA/log-dock.gif" 60

  echo "=== 10. command palette GIF ==="
  park_mouse
  focus_webview
  start_frames palette
  snap
  open_palette; sleep 0.4; snap
  sleep 0.6; snap
  xdotool type --delay 80 "workloads"; sleep 0.8; snap
  press_key Escape; sleep 0.4; snap
  build_gif "$MEDIA/command-palette.gif" 60
fi

stop_app

if [ -n "${NATIVE_ROUTE:-}" ]; then
  echo "=== 11. native route still + OS chrome ($NATIVE_ROUTE) ==="
  boot "$NATIVE_ROUTE" "native"
  park_mouse
  shoot "$MEDIA/native-logs-route.png"
  echo "native-logs-route.png bytes=$(stat -c%s "$MEDIA/native-logs-route.png" 2>/dev/null || echo 0)"

  if [ "$CAPTURE_FLOWS" = "1" ]; then
    focus_webview
    shoot "$MEDIA/native-menu-bar-shown.png"
    open_palette; sleep 0.6
    xdotool type --delay 80 "menu bar"; sleep 0.8
    press_key Return; sleep 1.2
    park_mouse
    shoot "$MEDIA/native-menu-bar-hidden.png"
    echo "menu_bar_ae=$(compare -metric AE "$MEDIA/native-menu-bar-shown.png" \
      "$MEDIA/native-menu-bar-hidden.png" null: 2>&1 | sed 's/[^0-9].*//' || echo 0)"

    for theme in light dark system; do
      open_palette; sleep 0.6
      xdotool type --delay 80 "title bar theme: $theme"; sleep 0.8
      press_key Return; sleep 1.2
      park_mouse
      shoot "$MEDIA/titlebar-$theme.png"
      echo "titlebar-$theme.png bytes=$(stat -c%s "$MEDIA/titlebar-$theme.png" 2>/dev/null || echo 0)"
    done
    echo "titlebar_light_dark_ae=$(compare -metric AE "$MEDIA/titlebar-light.png" \
      "$MEDIA/titlebar-dark.png" null: 2>&1 | sed 's/[^0-9].*//' || echo 0)"
  fi
  stop_app
fi

for pair in ${EXTRA_ROUTES:-}; do
  route="${pair%%:*}"
  slug="${pair#*:}"
  echo "=== extra route $route -> $slug.png ==="
  stop_app
  boot "$route" "$slug"
  park_mouse
  shoot "$MEDIA/$slug.png"
  echo "$slug.png bytes=$(stat -c%s "$MEDIA/$slug.png" 2>/dev/null || echo 0)"
  stop_app
done

echo "=== 12. media on the volume ==="
for f in "$MEDIA"/*; do
  [ -f "$f" ] || continue
  echo "MEDIA $(basename "$f") bytes=$(stat -c%s "$f")"
done
echo "=== CAPTURE COMPLETE ==="

#!/usr/bin/env bash
# OpenKite visual-capture wrapper.
#
# Applies the capture Job (dev/capture/capture-job.yaml.tmpl), waits for it,
# streams its log, and copies the media off the shared volume into docs/media/.
#
#   ./dev/capture/capture.sh
#
# Everything is overridable with OK_CAPTURE_* environment variables; run with
# `--help` for the list. The default namespace is the throwaway `ok-debug`, and
# the kubeconfig Secret is created out of band — see dev/capture/README.md. No
# credential is stored here, and nothing targets a GitOps-managed namespace:
# ArgoCD self-heal would revert (and fight) a manual Job.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DIR/../.." && pwd)"

NAMESPACE="${OK_CAPTURE_NAMESPACE:-ok-debug}"
SCRATCH_NS="${OK_CAPTURE_SCRATCH_NS:-$NAMESPACE}"
JOB_NAME="${OK_CAPTURE_JOB:-openkite-capture}"
CONFIGMAP_NAME="${OK_CAPTURE_CONFIGMAP:-openkite-capture-script}"
IMAGE="${OK_CAPTURE_IMAGE:-rust:1.98}"
PVC_NAME="${OK_CAPTURE_PVC:-openkite-cargo-cache}"
KUBECONFIG_SECRET="${OK_CAPTURE_KUBECONFIG_SECRET:-openkite-kubeconfig}"
BRANCH="${OK_CAPTURE_BRANCH:-main}"
SETTLE_S="${OK_CAPTURE_SETTLE_S:-50}"
SAMPLE_PODS="${OK_CAPTURE_SAMPLE_PODS:-4}"
DATA_MOUNT="${OK_CAPTURE_DATA_MOUNT:-/data}"
MEDIA_DIR="${OK_CAPTURE_MEDIA_DIR:-$DATA_MOUNT/media}"
SHELL_ROUTE="${OK_CAPTURE_SHELL_ROUTE:-/}"
CAPTURE_FLOWS="${OK_CAPTURE_FLOWS:-1}"
NATIVE_ROUTE="${OK_CAPTURE_NATIVE_ROUTE:-/logs}"
EXTRA_ROUTES="${OK_CAPTURE_EXTRA_ROUTES:-}"
WIN_W="${OK_CAPTURE_WIN_W:-1440}"
WIN_H="${OK_CAPTURE_WIN_H:-900}"
DEST="${OK_CAPTURE_DEST:-$REPO_ROOT/docs/media}"
LOG="${OK_CAPTURE_LOG:-$REPO_ROOT/.capture.log}"
FETCH_POD="${JOB_NAME}-fetch"
TIMEOUT="${OK_CAPTURE_TIMEOUT:-2400s}"

usage() {
  cat <<'EOF'
Usage: dev/capture/capture.sh [--help]

Renders and runs the capture Job, then fetches the media into docs/media/.
Environment (all optional):
  OK_CAPTURE_NAMESPACE            throwaway Job namespace      (default ok-debug)
  OK_CAPTURE_SCRATCH_NS           namespace for sample pods     (default = namespace)
  OK_CAPTURE_JOB                  Job / ConfigMap base name     (default openkite-capture)
  OK_CAPTURE_IMAGE                build image                   (default rust:1.98)
  OK_CAPTURE_PVC                  cargo-cache PVC name          (default openkite-cargo-cache)
  OK_CAPTURE_KUBECONFIG_SECRET    Secret holding the kubeconfig (default openkite-kubeconfig)
  OK_CAPTURE_KUBECONFIG           local kubeconfig file to seed the Secret from
  OK_CAPTURE_BRANCH               branch to build               (default main)
  OK_CAPTURE_SETTLE_S             seconds to wait for reflectors (default 50)
  OK_CAPTURE_SAMPLE_PODS          scratch pods for the table GIF (default 4)
  OK_CAPTURE_DATA_MOUNT           volume mount path in-container (default /data)
  OK_CAPTURE_MEDIA_DIR            in-container media directory   (default <mount>/media)
  OK_CAPTURE_SHELL_ROUTE          route for the console capture  (default /, the React console)
  OK_CAPTURE_FLOWS                1 to capture the interaction GIFs, 0 for stills only
  OK_CAPTURE_NATIVE_ROUTE         legacy native route still, "" to skip (default /logs)
  OK_CAPTURE_EXTRA_ROUTES         space-separated route:slug stills (default none)
  OK_CAPTURE_WIN_W                app window width in px        (default 1440)
  OK_CAPTURE_WIN_H                app window height in px       (default 900)
  OK_CAPTURE_DEST                 host destination dir          (default docs/media)
  OK_CAPTURE_LOG                  host log file                 (default .capture.log)
  OK_CAPTURE_TIMEOUT              kubectl wait timeout          (default 2400s)
EOF
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

command -v kubectl >/dev/null 2>&1 || { echo "kubectl is required" >&2; exit 1; }

# Upsert the payload ConfigMap so edits to capture-script.sh take effect.
kubectl -n "$NAMESPACE" create configmap "$CONFIGMAP_NAME" \
  --from-file=capture-script.sh="$DIR/capture-script.sh" \
  --dry-run=client -o yaml | kubectl apply -f - >/dev/null

# Seed the kubeconfig Secret when missing and a local file was provided.
if ! kubectl -n "$NAMESPACE" get secret "$KUBECONFIG_SECRET" >/dev/null 2>&1; then
  if [ -n "${OK_CAPTURE_KUBECONFIG:-}" ]; then
    kubectl -n "$NAMESPACE" create secret generic "$KUBECONFIG_SECRET" \
      --from-file=config="$OK_CAPTURE_KUBECONFIG"
  else
    echo "Secret '$KUBECONFIG_SECRET' not found in '$NAMESPACE'." >&2
    echo "Create it first, or set OK_CAPTURE_KUBECONFIG=<path> to seed it." >&2
    exit 1
  fi
fi

render() {
  sed \
    -e "s|@@NAMESPACE@@|${NAMESPACE}|g" \
    -e "s|@@JOB_NAME@@|${JOB_NAME}|g" \
    -e "s|@@CONFIGMAP_NAME@@|${CONFIGMAP_NAME}|g" \
    -e "s|@@IMAGE@@|${IMAGE}|g" \
    -e "s|@@BRANCH@@|${BRANCH}|g" \
    -e "s|@@SETTLE_S@@|${SETTLE_S}|g" \
    -e "s|@@SAMPLE_PODS@@|${SAMPLE_PODS}|g" \
    -e "s|@@SCRATCH_NS@@|${SCRATCH_NS}|g" \
    -e "s|@@DATA_MOUNT@@|${DATA_MOUNT}|g" \
    -e "s|@@MEDIA_DIR@@|${MEDIA_DIR}|g" \
    -e "s|@@SHELL_ROUTE@@|${SHELL_ROUTE}|g" \
    -e "s|@@CAPTURE_FLOWS@@|${CAPTURE_FLOWS}|g" \
    -e "s|@@NATIVE_ROUTE@@|${NATIVE_ROUTE}|g" \
    -e "s|@@EXTRA_ROUTES@@|${EXTRA_ROUTES}|g" \
    -e "s|@@WIN_W@@|${WIN_W}|g" \
    -e "s|@@WIN_H@@|${WIN_H}|g" \
    -e "s|@@PVC_NAME@@|${PVC_NAME}|g" \
    -e "s|@@KUBECONFIG_SECRET@@|${KUBECONFIG_SECRET}|g" \
    "$DIR/capture-job.yaml.tmpl"
}

echo "namespace=$NAMESPACE scratch=$SCRATCH_NS branch=$BRANCH shell=$SHELL_ROUTE native=$NATIVE_ROUTE"
kubectl -n "$NAMESPACE" delete job "$JOB_NAME" --ignore-not-found >/dev/null 2>&1 || true
render | kubectl apply -f -

echo "waiting for job/$JOB_NAME (timeout $TIMEOUT)..."
if ! kubectl -n "$NAMESPACE" wait --for=condition=complete "job/$JOB_NAME" --timeout="$TIMEOUT"; then
  echo "job did not complete; tailing logs:" >&2
  kubectl -n "$NAMESPACE" logs "job/$JOB_NAME" --tail=200 >&2 || true
  exit 1
fi

kubectl -n "$NAMESPACE" logs "job/$JOB_NAME" >"$LOG"
echo "wrote $LOG"
grep -E "GIF |bytes=|max_ae=|table_rows_|settled_|panics|CAPTURE COMPLETE" "$LOG" || true

echo "fetching media into $DEST ..."
kubectl -n "$NAMESPACE" delete pod "$FETCH_POD" --ignore-not-found >/dev/null 2>&1 || true
cat <<EOF | kubectl apply -f - >/dev/null
apiVersion: v1
kind: Pod
metadata:
  name: ${FETCH_POD}
  namespace: ${NAMESPACE}
spec:
  restartPolicy: Never
  containers:
    - name: fetch
      image: alpine:3.20
      command: ["sleep", "600"]
      volumeMounts:
        - { name: data, mountPath: ${DATA_MOUNT} }
  volumes:
    - name: data
      persistentVolumeClaim:
        claimName: ${PVC_NAME}
EOF
kubectl -n "$NAMESPACE" wait --for=condition=Ready "pod/$FETCH_POD" --timeout=120s
mkdir -p "$DEST"
# Fetch with `kubectl cp`, NOT by decoding the pod log: the capture script runs
# under `set -x`, whose trace interleaves with any base64 on stdout and makes
# the block undecodable ("number of data characters cannot be 1 more than a
# multiple of 4"). The shared volume is the source of truth.
kubectl cp "$NAMESPACE/$FETCH_POD:$MEDIA_DIR/." "$DEST/" 2>/dev/null || {
  for f in console-shell.png console-workloads.png native-logs-route.png \
           resource-table.gif inspector.gif toast.gif log-dock.gif command-palette.gif; do
    kubectl cp "$NAMESPACE/$FETCH_POD:$MEDIA_DIR/$f" "$DEST/$f" 2>/dev/null || true
  done
}
kubectl -n "$NAMESPACE" delete pod "$FETCH_POD" --ignore-not-found >/dev/null 2>&1 || true

echo "media -> $DEST"
ls -la "$DEST"

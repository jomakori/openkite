#!/usr/bin/env bash
#
# report-main-failure.sh — open or update exactly ONE tracking issue for a
# workflow run that failed on main.
#
# A merge to main can leave the required-check set green while a workflow it
# triggered — notably the path-filtered `Release` — is red and unseen. This
# turns such a failure into an issue naming the workflow, the failing job(s),
# and the log link, and updates the SAME issue on every later failure instead of
# filing a duplicate. Idempotency is keyed on the issue title, which depends
# only on the workflow name.
#
# Environment:
#   GITHUB_REPOSITORY   owner/repo (set by Actions)
#   RUN_ID              numeric Actions run id to report
#   WORKFLOW_NAME       workflow to attribute the failure to (optional; falls
#                       back to the run's own name)
#   REPORT_DRY_RUN      'true' prints what would happen without writing
#   GH_TOKEN            token with issues:write
set -euo pipefail

repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
run_id="${RUN_ID:?RUN_ID is required}"
dry="${REPORT_DRY_RUN:-false}"

case "$run_id" in
  '' | *[!0-9]*)
    echo "::error title=Invalid run id::RUN_ID must be numeric, got '${run_id}'" >&2
    exit 1
    ;;
esac

IFS=$'\t' read -r run_name run_url head_sha head_branch attempt conclusion < <(
  gh api "repos/${repo}/actions/runs/${run_id}" \
    --jq '[.name, .html_url, .head_sha, (.head_branch // "unknown"), (.run_attempt | tostring), (.conclusion // "failure")] | @tsv'
)
workflow="${WORKFLOW_NAME:-$run_name}"

# shellcheck disable=SC2016  # jq program is intentionally single-quoted
failing_md="$(gh api "repos/${repo}/actions/runs/${run_id}/jobs?per_page=100" \
  --jq '[.jobs[] | select(.conclusion == "failure" or .conclusion == "timed_out")]
        | if length == 0
          then "_(no individual job reported a failure; the run itself failed)_"
          else map("- `" + .name + "` — [logs](" + .html_url + ")") | join("\n")
          end')"

title="[ci] ${workflow} is red on main"
marker="<!-- main-failure-tracker:${workflow} -->"
now="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"

body="## Latest failure — ${now}

| | |
|---|---|
| Workflow | \`${workflow}\` |
| Conclusion | \`${conclusion}\` |
| Run | [attempt ${attempt}](${run_url}) |
| Commit | \`${head_sha}\` |
| Branch | \`${head_branch}\` |

**Failing job(s)**

${failing_md}

A green *required-check* set is not evidence that the workflows a merge
triggered actually succeeded — a \`success\` status can hide a skipped or unrun
job. Check \`Release\` first. See the [release runbook](https://github.com/${repo}/blob/main/docs/release-runbook.md)
and the [workflow reference](https://github.com/${repo}/blob/main/.github/workflows/README.md).

${marker}"

comment="### Re-failed — ${now}

- Workflow: \`${workflow}\`
- Conclusion: \`${conclusion}\`
- Run: [attempt ${attempt}](${run_url})
- Commit: \`${head_sha}\`
- Failing job(s):

${failing_md}"

export TRACKER_TITLE="$title"
existing="$(gh api "repos/${repo}/issues?state=open&labels=ci-failure&per_page=100" \
  --jq '[.[] | select(.pull_request == null) | select(.title == env.TRACKER_TITLE)][0].number // empty')"

if [ "$dry" = "true" ]; then
  if [ -n "$existing" ]; then
    echo "DRY RUN: would update issue #${existing}"
  else
    echo "DRY RUN: would create issue '${title}'"
  fi
  printf '%s\n' "$body"
  exit 0
fi

if [ -n "$existing" ]; then
  gh issue comment "$existing" --repo "$repo" --body "$comment" >/dev/null
  echo "updated existing tracking issue: https://github.com/${repo}/issues/${existing}"
else
  if ! gh label list --repo "$repo" --limit 100 --json name --jq '.[].name' | grep -qx 'ci-failure'; then
    gh label create ci-failure --repo "$repo" --color B60205 \
      --description "A workflow failed on main (automated)"
  fi
  gh issue create --repo "$repo" --title "$title" --label ci-failure --body "$body"
fi

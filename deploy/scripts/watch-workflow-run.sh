#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Trigger a GitHub Actions workflow_dispatch run (or attach to an
# already-running one), poll it to completion, and on failure print the
# failed step's log tail plus any "Dump pod logs on failure"-style
# diagnostics step output, if the workflow has one (see k3s-e2e.yml).
#
# Polling is robust to transient `gh` API network hiccups — a single
# failed poll retries instead of killing the whole watch.
#
# Usage:
#   watch-workflow-run.sh <workflow-file> [ref]   # trigger + watch (ref defaults to main)
#   watch-workflow-run.sh --run-id <run-id>        # watch an already-running run
#
# Examples:
#   deploy/scripts/watch-workflow-run.sh k3s-e2e.yml
#   deploy/scripts/watch-workflow-run.sh k3s-e2e.yml my-branch
#   deploy/scripts/watch-workflow-run.sh --run-id 31877288435
#
# Exit code matches the run's outcome: 0 on success, 1 otherwise — usable
# in a loop / CI gate, not just interactively.
set -uo pipefail

REPO="${REPO:-hypersdk/guestkit}"

if [[ "${1:-}" == "--run-id" ]]; then
  RUN_ID="${2:?usage: watch-workflow-run.sh --run-id <run-id>}"
else
  WORKFLOW="${1:?usage: watch-workflow-run.sh <workflow-file> [ref] | --run-id <run-id>}"
  REF="${2:-main}"
  echo "Triggering ${WORKFLOW} on ${REPO}@${REF}..."
  gh workflow run "${WORKFLOW}" --repo "${REPO}" --ref "${REF}"
  # workflow_dispatch doesn't return a run ID directly — poll the run list
  # until the new run shows up (it's the newest one right after dispatch).
  sleep 6
  RUN_ID=$(gh run list --repo "${REPO}" --workflow "${WORKFLOW}" --limit 1 --json databaseId --jq '.[0].databaseId')
fi

echo "Run ID: ${RUN_ID}"
echo "https://github.com/${REPO}/actions/runs/${RUN_ID}"

prev=""
run_conclusion=""
while true; do
  data=$(gh run view "${RUN_ID}" --repo "${REPO}" --json status,conclusion,jobs 2>&1) || {
    echo "  (transient gh API error, retrying) ${data}" >&2
    sleep 30
    continue
  }
  cur=$(echo "${data}" | jq -r '.jobs[] | "\(.name): \(.status) \(.conclusion)"' 2>/dev/null || true)
  if [[ "${cur}" != "${prev}" ]]; then
    echo "  ${cur}"
    prev="${cur}"
  fi
  run_status=$(echo "${data}" | jq -r '.status' 2>/dev/null || echo "")
  if [[ "${run_status}" == "completed" ]]; then
    run_conclusion=$(echo "${data}" | jq -r '.conclusion' 2>/dev/null || echo "")
    echo "TERMINAL: ${run_conclusion}"
    break
  fi
  sleep 60
done

if [[ "${run_conclusion}" != "success" ]]; then
  echo ""
  echo "=== Pod log dump (if this workflow has a 'Dump pod logs on failure' step) ==="
  gh run view "${RUN_ID}" --repo "${REPO}" --log 2>&1 \
    | awk -F'\t' '$2 == "Dump pod logs on failure" { $1=$2=""; print }'
  echo ""
  echo "=== Last 40 lines of the failed step ==="
  gh run view "${RUN_ID}" --repo "${REPO}" --log-failed 2>&1 | tail -40
fi

echo ""
echo "Run URL: https://github.com/${REPO}/actions/runs/${RUN_ID}"
[[ "${run_conclusion}" == "success" ]]

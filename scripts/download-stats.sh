#!/usr/bin/env bash
#
# Report release-asset download counts for an explicitly selected repository.
# Downloads are a rough trend, not unique installs: retries and CI count too;
# source builds and uninstalls are invisible.
#
# USAGE:
#   REPO=owner/name scripts/download-stats.sh              # per-release totals
#   REPO=owner/name scripts/download-stats.sh --by-asset    # totals per asset
#   REPO=owner/name scripts/download-stats.sh --total       # grand total
#
# REQUIRES: the GitHub CLI (`gh`), authenticated (`gh auth login`). We use
# `gh api --jq`, whose embedded jq means no external `jq` install is needed.
# Unauthenticated access hits a low rate limit, so `gh` auth is expected.

set -euo pipefail

: "${REPO:?Set REPO=OWNER/REPOSITORY for this project}"
MODE="${1:-per-release}"

if ! command -v gh >/dev/null 2>&1; then
  echo "error: the GitHub CLI (gh) is required. Install it and run 'gh auth login'." >&2
  echo "       https://cli.github.com" >&2
  exit 1
fi

# One API call, paginated, flattened to TSV. Exclude checksum sidecars so an
# archive and its checksum do not count as two product downloads.
ARCHIVE_ONLY='select((.name | endswith(".sha256")) | not)'

release_rows() {
  gh api --paginate "repos/${REPO}/releases" \
    --jq ".[] | [.tag_name, ((.published_at // \"\") | .[0:10]), ([.assets[] | ${ARCHIVE_ONLY} | .download_count] | add // 0)] | @tsv"
}

asset_rows() {
  gh api --paginate "repos/${REPO}/releases" \
    --jq ".[].assets[] | ${ARCHIVE_ONLY} | [.name, .download_count] | @tsv"
}

case "$MODE" in
  --total)
    release_rows | awk -F'\t' '{ t += $3 } END { print t + 0 }'
    ;;

  --by-asset)
    # Sum download_count per asset name across every release, biggest first.
    asset_rows | awk -F'\t' '
      { count[$1] += $2; total += $2 }
      END {
        printf "%-52s %10s\n", "ASSET", "DOWNLOADS"
        # crude descending sort by value
        n = 0
        for (k in count) { keys[n] = k; n++ }
        for (i = 0; i < n; i++)
          for (j = i + 1; j < n; j++)
            if (count[keys[j]] > count[keys[i]]) { tmp = keys[i]; keys[i] = keys[j]; keys[j] = tmp }
        for (i = 0; i < n; i++) printf "%-52s %10d\n", keys[i], count[keys[i]]
        printf "%-52s %10d\n", "TOTAL", total + 0
      }'
    ;;

  per-release|"")
    release_rows | awk -F'\t' '
      BEGIN { printf "%-16s %-12s %10s\n", "RELEASE", "PUBLISHED", "DOWNLOADS" }
      { printf "%-16s %-12s %10d\n", $1, $2, $3; total += $3 }
      END { printf "%-16s %-12s %10d\n", "TOTAL", "", total + 0 }'
    ;;

  *)
    echo "usage: $0 [--by-asset | --total]" >&2
    exit 2
    ;;
esac

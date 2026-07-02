#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -gt 0 ] && [ "${1-}" != "-" ]; then
  release_payload="$1"
elif [ ! -t 0 ] || [ "${1-}" = "-" ]; then
  release_payload="$(cat)"
  if [ -z "${release_payload}" ]; then
    echo "usage: summarise-release-please.sh '<release-please outputs json>'" >&2
    echo "   or: summarise-release-please.sh < <json>" >&2
    exit 1
  fi
else
  echo "usage: summarise-release-please.sh '<release-please outputs json>'" >&2
  echo "   or: summarise-release-please.sh < <json>" >&2
  exit 1
fi
mode="${RP_MODE:-unknown}"
config_file="${RP_CONFIG_FILE:-unknown}"

jq -e . >/dev/null <<<"${release_payload}"
releases_created="$(jq -r '.releases_created // "false"' <<<"${release_payload}")"
prs_created="$(jq -r '.prs_created // "false"' <<<"${release_payload}")"
paths_released="$(jq -r '.paths_released // "[]"' <<<"${release_payload}")"
prs="$(jq -r '.prs // "[]"' <<<"${release_payload}")"

{
  echo "## Release Please Outputs"
  echo
  echo "| field | value |"
  echo "|---|---|"
  echo "| mode | \`${mode}\` |"
  echo "| config_file | \`${config_file}\` |"
  echo "| releases_created | \`${releases_created}\` |"
  echo "| prs_created | \`${prs_created}\` |"
  echo "| publish_units_source | \`moon query projects --affected\` |"
  echo "| mode_signal | \`releases_created=true => latest, else next\` |"

  if [ "${releases_created}" = "true" ]; then
    echo
    echo "### Released paths (informational only)"
    jq -r '(. // "[]") | (fromjson? // .) | .[]? | "- \(.)"' <<<"${paths_released}"
  fi

  echo
  echo "_Publish units come from moon affected query. release-please paths are informational only. releases_created is used only as the mode/tag signal._"

  if [ "${prs_created}" = "true" ]; then
    echo
    echo "### Release PRs"
    jq -r '(. // "[]") | (fromjson? // .) | .[]? | "- #\(.number): \(.title // "")"' <<<"${prs}"
  fi

  echo
  echo "<details><summary>raw outputs json</summary>"
  echo
  echo '```json'
  jq . <<<"${release_payload}"
  echo '```'
  echo
  echo "</details>"
} >> "${GITHUB_STEP_SUMMARY}"

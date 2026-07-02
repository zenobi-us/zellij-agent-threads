#!/usr/bin/env bash
set -euo pipefail

require_tools() {
  command -v moon >/dev/null || {
    echo "moon not found" >&2
    exit 1
  }
  command -v jq >/dev/null || {
    echo "jq not found" >&2
    exit 1
  }
  command -v diff >/dev/null || {
    echo "diff not found" >&2
    exit 1
  }
}

discover_release_files() {
  shopt -s nullglob
  configs=(release-please-config--*.json)
  shopt -u nullglob

  if [[ ${#configs[@]} -eq 0 ]]; then
    echo "No release-please-config--*.json files found." >&2
    exit 1
  fi

  manifest_file=".release-please-manifest.json"
  if [[ ! -f "${manifest_file}" ]]; then
    echo "${manifest_file} not found." >&2
    exit 1
  fi
}

# 1) moon query + package.json > release package map
moon_query_to_project_map() {
  moon query projects | jq -r '
    .projects[]?
    | select((.source | strings) != "")
    | [.source, (.layer // .config.layer // "unknown")]
    | @tsv
  ' | while IFS=$'\t' read -r source layer; do
    local package_file="${source}/package.json"

    if [[ ! -f "${package_file}" ]]; then
      continue
    fi

    if ! jq -e '.private != true and (.publishConfig.access // "") != ""' "${package_file}" >/dev/null; then
      continue
    fi

    local version
    version="$(jq -r '.version // "0.1.0"' "${package_file}")"

    local group="${layer}"
    if [[ "${source}" == pkgs/provider-* ]]; then
      group="provider"
    fi

    jq -nc --arg source "${source}" --arg group "${group}" --arg version "${version}" '{
      key: $source,
      value: {
        group: $group,
        "release-type": "node",
        version: $version
      }
    }'
  done | jq -sc '{ packages: (sort_by(.key) | from_entries) }'
}

# 2) 1 > rp-config.packages format
moon_query_to_rp_config_packages() {
  local project_map_json="${1}"
  jq -c '
    (.packages // {})
    | with_entries(del(.value.version))
    | to_entries
    | sort_by(.key)
    | from_entries
  ' <<<"${project_map_json}"
}

# 3) 1 > rp-manifest format
# Preserves existing versions when present; otherwise uses package.json version.
moon_query_to_rp_manifest() {
  local project_map_json="${1}"
  local existing_manifest_json="${2:-}"
  if [[ -z "${existing_manifest_json}" ]]; then
    existing_manifest_json='{}'
  fi

  jq -c --argjson existing "${existing_manifest_json}" '
    (.packages // {} | to_entries | sort_by(.key)) as $packages
    | reduce $packages[] as $package ({};
        .[$package.key] = ($existing[$package.key] // $package.value.version // "0.1.0")
      )
  ' <<<"${project_map_json}"
}

print_key_diff() {
  local label="${1}"
  local expected_json="${2}"
  local actual_json="${3}"

  local expected_file actual_file
  expected_file="$(mktemp)"
  actual_file="$(mktemp)"

  jq -r 'keys[]' <<<"${expected_json}" >"${expected_file}"
  jq -r 'keys[]' <<<"${actual_json}" >"${actual_file}"

  if ! diff -u "${actual_file}" "${expected_file}" >/dev/null; then
    echo "${label} key mismatch" >&2
    diff -u "${actual_file}" "${expected_file}" >&2 || true
    rm -f "${expected_file}" "${actual_file}"
    return 1
  fi

  rm -f "${expected_file}" "${actual_file}"
}

run_check() {
  discover_release_files

  local project_map expected_packages existing_manifest expected_manifest
  project_map="$(moon_query_to_project_map)"
  expected_packages="$(moon_query_to_rp_config_packages "${project_map}")"
  existing_manifest="$(jq -c '.' "${manifest_file}")"
  expected_manifest="$(moon_query_to_rp_manifest "${project_map}" "${existing_manifest}")"

  local failed=0

  if ! print_key_diff \
    "Manifest (.release-please-manifest.json)" \
    "${expected_manifest}" \
    "${existing_manifest}"; then
    failed=1
  fi

  local config actual_packages
  for config in "${configs[@]}"; do
    actual_packages="$(jq -c '.packages // {}' "${config}")"
    if ! print_key_diff "Config (${config}) .packages" "${expected_packages}" "${actual_packages}"; then
      failed=1
    fi
  done

  if [[ "${failed}" -ne 0 ]]; then
    exit 1
  fi

  echo "release-please manifest/config keys are in sync with moon query output."
}

run_sync() {
  discover_release_files

  local project_map expected_packages existing_manifest expected_manifest
  project_map="$(moon_query_to_project_map)"
  expected_packages="$(moon_query_to_rp_config_packages "${project_map}")"
  existing_manifest="$(jq -c '.' "${manifest_file}")"
  expected_manifest="$(moon_query_to_rp_manifest "${project_map}" "${existing_manifest}")"

  local tmp config

  echo "Syncing ${manifest_file}"
  tmp="$(mktemp)"
  jq '.' <<<"${expected_manifest}" >"${tmp}"
  mv "${tmp}" "${manifest_file}"

  for config in "${configs[@]}"; do
    echo "Syncing ${config}"
    tmp="$(mktemp)"
    jq --argjson packages "${expected_packages}" '
      .packages = $packages
      | .packages |= (to_entries | sort_by(.key) | from_entries)
    ' "${config}" >"${tmp}"
    mv "${tmp}" "${config}"
  done
}

usage() {
  cat <<'EOF'
Usage:
  action.sh check   # compare moon-derived keys vs release-please files
  action.sh sync    # rewrite release-please files from moon-derived data

Default command: sync
EOF
}

main() {
  require_tools

  local cmd="${1:-sync}"
  case "${cmd}" in
  check)
    run_check
    ;;
  sync)
    run_sync
    ;;
  -h | --help | help)
    usage
    ;;
  *)
    echo "Unknown command: ${cmd}" >&2
    usage >&2
    exit 2
    ;;
  esac
}

main "$@"

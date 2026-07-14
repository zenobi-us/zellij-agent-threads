#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/action.sh"
  WORKSPACE="$(mktemp -d)"
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"

  printf '{"packages":{},"plugins":[]}\n' >"${WORKSPACE}/release-please-config--release.json"
  printf '{"packages":{},"plugins":[]}\n' >"${WORKSPACE}/release-please-config--hotfix.json"
  printf '{"apps/node":"9.9.9"}\n' >"${WORKSPACE}/.release-please-manifest.json"
  mkdir -p "${WORKSPACE}/apps/node" "${WORKSPACE}/pkgs/rust" "${WORKSPACE}/tools/internal"
  printf '{"name":"node","version":"1.2.3"}\n' >"${WORKSPACE}/apps/node/package.json"
  printf '[package]\nname="rust"\nversion="2.3.4"\n' >"${WORKSPACE}/pkgs/rust/Cargo.toml"

  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${MOON_PROJECTS_JSON}"
EOF
  cat >"${MOCK_DIR}/cargo" <<EOF
#!/usr/bin/env bash
printf '{"packages":[{"manifest_path":"${WORKSPACE}/pkgs/rust/Cargo.toml","version":"2.3.4"}]}\n'
EOF
  chmod +x "${MOCK_DIR}/moon" "${MOCK_DIR}/cargo"

  export MOON_PROJECTS_JSON='{"projects":[
    {"id":"node","source":"apps/node","layer":"application","tasks":{"publish":{}}},
    {"id":"rust","source":"pkgs/rust","layer":"library","tasks":{"publish":{}}},
    {"id":"internal","source":"tools/internal","layer":"tool","tasks":{"test":{}}}
  ]}'
}

teardown() {
  rm -rf "${WORKSPACE}" "${MOCK_DIR}"
}

run_action() {
  run bash -c 'cd "$1" && "$2" "$3"' _ "${WORKSPACE}" "${SCRIPT}" "$1"
}

@test "sync discovers Node and Cargo publish projects and excludes non-publishable projects" {
  run_action sync
  [ "$status" -eq 0 ]

  run jq -e '.packages == {
    "apps/node":{"component":"node","group":"application","release-type":"node"},
    "pkgs/rust":{"component":"rust","group":"library","release-type":"rust"}
  }' "${WORKSPACE}/release-please-config--release.json"
  [ "$status" -eq 0 ]
  run jq -e 'has("tools/internal") | not' "${WORKSPACE}/release-please-config--release.json"
  [ "$status" -eq 0 ]
}

@test "sync preserves existing manifest versions and seeds new versions" {
  run_action sync
  [ "$status" -eq 0 ]
  run jq -e '. == {"apps/node":"9.9.9","pkgs/rust":"2.3.4"}' "${WORKSPACE}/.release-please-manifest.json"
  [ "$status" -eq 0 ]
}

@test "normal and hotfix configs receive identical package keys and components" {
  run_action sync
  [ "$status" -eq 0 ]
  run bash -c 'diff -u <(jq -S .packages "$1") <(jq -S .packages "$2")' _ \
    "${WORKSPACE}/release-please-config--release.json" \
    "${WORKSPACE}/release-please-config--hotfix.json"
  [ "$status" -eq 0 ]
}

@test "unsupported publishable project fails synchronization" {
  export MOON_PROJECTS_JSON='{"projects":[{"id":"unknown","source":"tools/internal","tasks":{"publish":{}}}]}'
  run_action sync
  [ "$status" -ne 0 ]
  [[ "$output" == *"Unsupported version source"* ]]
}

@test "check rejects wrong component, release type, and manifest version" {
  run_action sync
  [ "$status" -eq 0 ]
  jq '.packages["apps/node"].component = "wrong" | .packages["apps/node"]["release-type"] = "rust"' "${WORKSPACE}/release-please-config--release.json" >"${WORKSPACE}/config.tmp"
  mv "${WORKSPACE}/config.tmp" "${WORKSPACE}/release-please-config--release.json"
  jq '.["pkgs/rust"] = "0.0.0"' "${WORKSPACE}/.release-please-manifest.json" >"${WORKSPACE}/manifest.tmp"
  mv "${WORKSPACE}/manifest.tmp" "${WORKSPACE}/.release-please-manifest.json"

  run_action check
  [ "$status" -ne 0 ]
  [[ "$output" == *"mismatch"* ]]
}

#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/resolve-publish-metadata"
  WORKSPACE="$(mktemp -d)"
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"
  export MOON_WORKSPACE_ROOT="${WORKSPACE}"

  git -C "${WORKSPACE}" init -q
  git -C "${WORKSPACE}" config user.email test@example.com
  git -C "${WORKSPACE}" config user.name Test
  git -C "${WORKSPACE}" checkout -q -b main
  mkdir -p "${WORKSPACE}/pkg"
  printf '{"name":"fixture","version":"1.2.3"}\n' >"${WORKSPACE}/pkg/package.json"
  git -C "${WORKSPACE}" add .
  git -C "${WORKSPACE}" commit -q -m root

  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$*" == "query projects" ]] || exit 1
jq -nc --arg source "${MOON_SOURCE:-pkg}" --arg version_task "${MOON_PUBLISHABLE:-true}" '
  {projects:[{id:"fixture",source:$source,tasks:(if $version_task == "true" then {publish:{}} else {} end)}]}'
EOF
  chmod +x "${MOCK_DIR}/moon" "${SCRIPT}"
}

teardown() {
  rm -rf "${WORKSPACE}" "${MOCK_DIR}"
}

commit_empty() {
  git -C "${WORKSPACE}" commit -q --allow-empty -m "$1"
}

@test "latest preserves Node version and component tag" {
  run "${SCRIPT}" fixture latest main 1
  [ "$status" -eq 0 ]
  run jq -e '.current_version == "1.2.3" and .version == "1.2.3" and .release_tag == "pkg-v1.2.3"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "Cargo version resolves through metadata" {
  rm "${WORKSPACE}/pkg/package.json"
  printf '[package]\nname="fixture"\nversion="2.3.4"\n' >"${WORKSPACE}/pkg/Cargo.toml"
  cat >"${MOCK_DIR}/cargo" <<EOF
#!/usr/bin/env bash
printf '{"packages":[{"manifest_path":"${WORKSPACE}/pkg/Cargo.toml","version":"2.3.4"}]}\n'
EOF
  chmod +x "${MOCK_DIR}/cargo"

  run "${SCRIPT}" fixture latest main 1
  [ "$status" -eq 0 ]
  run jq -e '.current_version == "2.3.4" and .release_tag == "pkg-v2.3.4"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "next uses stable first-parent distance and ignores prerelease tags" {
  git -C "${WORKSPACE}" tag pkg-v1.2.3
  commit_empty one
  git -C "${WORKSPACE}" tag pkg-v9.0.0-next.1.1
  commit_empty two

  run "${SCRIPT}" fixture next main 4
  [ "$status" -eq 0 ]
  run jq -e '.stable_tag == "pkg-v1.2.3" and .commit_distance == 2 and .version == "1.3.0-next.2.4"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "release branch bumps patch and retry changes final identifier only" {
  commit_empty one
  run "${SCRIPT}" fixture next release/1.2 1
  [ "$status" -eq 0 ]
  first="$output"
  run "${SCRIPT}" fixture next release/1.2 2
  [ "$status" -eq 0 ]
  second="$output"
  run jq -en --argjson first "$first" --argjson second "$second" '$first.version == "1.2.4-next.2.1" and $second.version == "1.2.4-next.2.2" and $first.commit_distance == $second.commit_distance'
  [ "$status" -eq 0 ]
}

@test "missing stable tag counts first-parent history from root deterministically" {
  commit_empty one
  commit_empty two
  run "${SCRIPT}" fixture next main 1
  [ "$status" -eq 0 ]
  first="$output"
  run "${SCRIPT}" fixture next main 1
  [ "$status" -eq 0 ]
  [ "$output" = "$first" ]
  run jq -e '.commit_distance == 3 and .version == "1.3.0-next.3.1"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "root component uses v-prefixed release tag" {
  export MOON_SOURCE='.'
  cp "${WORKSPACE}/pkg/package.json" "${WORKSPACE}/package.json"
  run "${SCRIPT}" fixture latest main 1
  [ "$status" -eq 0 ]
  run jq -e '.source == "." and .release_tag == "v1.2.3"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "rejects malformed semantic versions" {
  for version in '1.0.0-01' '1.0.0-alpha..1' '1.0.0+meta..x'; do
    jq --arg version "$version" '.version = $version' "${WORKSPACE}/pkg/package.json" >"${WORKSPACE}/pkg/package.tmp"
    mv "${WORKSPACE}/pkg/package.tmp" "${WORKSPACE}/pkg/package.json"
    run "${SCRIPT}" fixture latest main 1
    [ "$status" -ne 0 ]
    [[ "$output" == *"malformed semantic version"* ]]
  done
}

@test "rejects unknown target, non-publishable target, and unsupported branch" {
  run "${SCRIPT}" missing latest main 1
  [ "$status" -ne 0 ]
  [[ "$output" == *"unknown Moon target"* ]]

  export MOON_PUBLISHABLE=false
  run "${SCRIPT}" fixture latest main 1
  [ "$status" -ne 0 ]
  [[ "$output" == *"not publishable"* ]]

  export MOON_PUBLISHABLE=true
  run "${SCRIPT}" fixture next feature/nope 1
  [ "$status" -ne 0 ]
  [[ "$output" == *"unsupported source branch"* ]]
}

#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/get-publish-matrix"
  BASE="abc123"
  HEAD="def456"
  PAYLOAD_FALSE='{"releases_created":"false"}'

  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"

  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" = "query" && "${2:-}" = "projects" && "${3:-}" = "--affected" ]]; then
  if [[ -n "${MOON_MOCK_ASSERT_BASE:-}" && "${MOON_BASE:-}" != "${MOON_MOCK_ASSERT_BASE}" ]]; then
    echo "unexpected MOON_BASE: ${MOON_BASE:-}" >&2
    exit 1
  fi

  if [[ -n "${MOON_MOCK_ASSERT_HEAD:-}" && "${MOON_HEAD:-}" != "${MOON_MOCK_ASSERT_HEAD}" ]]; then
    echo "unexpected MOON_HEAD: ${MOON_HEAD:-}" >&2
    exit 1
  fi

  output="${MOON_MOCK_OUTPUT:-}"
  if [[ -z "${output}" ]]; then
    output='{"projects":[]}'
  fi
  printf '%s\n' "${output}"
  exit 0
fi

echo "unexpected moon args: $*" >&2
exit 1
EOF

  chmod +x "${MOCK_DIR}/moon"
}

teardown() {
  rm -rf "${MOCK_DIR}"
}

@test "fails on missing payload" {
  run bash "${SCRIPT}"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Missing JSON payload argument"* ]]
}

@test "fails on invalid payload" {
  run bash "${SCRIPT}" '{not-json}' "${BASE}" "${HEAD}"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Invalid JSON payload passed as argv[0]"* ]]
}

@test "fails when base is missing" {
  run bash "${SCRIPT}" "${PAYLOAD_FALSE}" "" "${HEAD}"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Missing base argument"* ]]
}

@test "fails when head is missing" {
  run bash "${SCRIPT}" "${PAYLOAD_FALSE}" "${BASE}" ""
  [ "$status" -eq 1 ]
  [[ "$output" == *"Missing head argument"* ]]
}

@test "releases_created false resolves next mode/tag and filters publishable targets" {
  export MOON_MOCK_ASSERT_BASE="${BASE}"
  export MOON_MOCK_ASSERT_HEAD="${HEAD}"
  export MOON_MOCK_OUTPUT='{
    "projects": [
      { "id": "repo", "tasks": { "lint": {} } },
      { "id": "public-site", "tasks": { "publish": {}, "build": {} } },
      { "id": "features", "tasks": { "publish": {} } }
    ]
  }'

  run bash "${SCRIPT}" "${PAYLOAD_FALSE}" "${BASE}" "${HEAD}"
  [ "$status" -eq 0 ]

  run jq -e '. == [
    {"target":"features","tag":"next","mode":"next","version":""},
    {"target":"public-site","tag":"next","mode":"next","version":""}
  ]' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "releases_created true (boolean) resolves latest mode/tag" {
  export MOON_MOCK_OUTPUT='{
    "projects": [
      { "id": "features", "tasks": { "publish": {} } }
    ]
  }'

  run bash "${SCRIPT}" '{"releases_created":true}' "${BASE}" "${HEAD}"
  [ "$status" -eq 0 ]

  run jq -e '. == [
    {"target":"features","tag":"latest","mode":"latest","version":""}
  ]' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "accepts payload from @file to avoid argv size limits" {
  export MOON_MOCK_OUTPUT='{
    "projects": [
      { "id": "features", "tasks": { "publish": {} } }
    ]
  }'

  payload_file="$(mktemp)"
  cat >"${payload_file}" <<'EOF'
{"releases_created":"false","pr":"x","prs":"[]"}
EOF

  run bash "${SCRIPT}" "@${payload_file}" "${BASE}" "${HEAD}"
  rm -f "${payload_file}"

  [ "$status" -eq 0 ]
  run jq -e '. == [
    {"target":"features","tag":"next","mode":"next","version":""}
  ]' <<<"$output"
  [ "$status" -eq 0 ]
}
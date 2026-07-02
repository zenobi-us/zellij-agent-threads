#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/summarise-release-please.sh"
  FIXTURES_DIR="${BATS_TEST_DIRNAME}/fixtures/release-please"
  SUMMARY_FILE="$(mktemp)"
}

teardown() {
  rm -f "${SUMMARY_FILE}"
}

@test "summarise-release-please writes summary from stringified release-please fields" {
  payload="$(cat "${FIXTURES_DIR}/created-stringified.json")"

  run env \
    GITHUB_STEP_SUMMARY="${SUMMARY_FILE}" \
    RP_MODE="release" \
    RP_CONFIG_FILE="release-please-config--release.json" \
    bash "${SCRIPT}" "${payload}"

  [ "$status" -eq 0 ]

  summary="$(cat "${SUMMARY_FILE}")"
  [[ "${summary}" == *"## Release Please Outputs"* ]]
  [[ "${summary}" == *"| mode | \`release\` |"* ]]
  [[ "${summary}" == *"| publish_units_source | \`moon query projects --affected\` |"* ]]
  [[ "${summary}" == *"| mode_signal | \`releases_created=true => latest, else next\` |"* ]]
  [[ "${summary}" == *"### Released paths (informational only)"* ]]
  [[ "${summary}" == *"- apps/web"* ]]
  [[ "${summary}" == *"### Release PRs"* ]]
  [[ "${summary}" == *"Publish units come from moon affected query"* ]]
  [[ "${summary}" == *"- #42: chore(main): release"* ]]
}

@test "summarise-release-please handles native JSON arrays for paths and prs" {
  payload="$(cat "${FIXTURES_DIR}/created-native.json")"

  run env \
    GITHUB_STEP_SUMMARY="${SUMMARY_FILE}" \
    RP_MODE="hotfix" \
    RP_CONFIG_FILE="release-please-config--hotfix.json" \
    bash "${SCRIPT}" "${payload}"

  [ "$status" -eq 0 ]

  summary="$(cat "${SUMMARY_FILE}")"
  [[ "${summary}" == *"| mode | \`hotfix\` |"* ]]
  [[ "${summary}" == *"- apps/api"* ]]
  [[ "${summary}" == *"- pkgs/libs/core"* ]]
  [[ "${summary}" == *"- #100: fix: release api"* ]]
  [[ "${summary}" == *"- #101: fix: release core"* ]]
}

@test "summarise-release-please omits release and PR sections when none created" {
  payload="$(cat "${FIXTURES_DIR}/no-release.json")"

  run env \
    GITHUB_STEP_SUMMARY="${SUMMARY_FILE}" \
    RP_MODE="release" \
    RP_CONFIG_FILE="release-please-config--release.json" \
    bash "${SCRIPT}" "${payload}"

  [ "$status" -eq 0 ]

  summary="$(cat "${SUMMARY_FILE}")"
  [[ "${summary}" == *"| releases_created | \`false\` |"* ]]
  [[ "${summary}" == *"| prs_created | \`false\` |"* ]]
  [[ "${summary}" != *"### Released paths"* ]]
  [[ "${summary}" != *"### Release PRs"* ]]
}

@test "summarise-release-please reads release payload from stdin with multiline markdown/html in stringified PR body" {
  payload_file="${FIXTURES_DIR}/created-stringified-multiline-pr-body.json"

  run bash -c 'env GITHUB_STEP_SUMMARY="$1" RP_MODE="release" RP_CONFIG_FILE="release-please-config--release.json" bash "$2" < "$3"' _ "${SUMMARY_FILE}" "${SCRIPT}" "${payload_file}"

  [ "$status" -eq 0 ]

  summary="$(cat "${SUMMARY_FILE}")"
  [[ "${summary}" == *"### Release PRs"* ]]
  [[ "${summary}" == *"- #314: chore(main): release 1.2.3"* ]]
}

@test "summarise-release-please fails on invalid json payload" {
  payload="$(cat "${FIXTURES_DIR}/invalid.json")"

  run env GITHUB_STEP_SUMMARY="${SUMMARY_FILE}" bash "${SCRIPT}" "${payload}"

  [ "$status" -ne 0 ]
  [[ "$output" == *"parse error"* ]]
}

@test "summarise-release-please fails when payload argument is missing" {
  run env GITHUB_STEP_SUMMARY="${SUMMARY_FILE}" bash "${SCRIPT}"

  [ "$status" -ne 0 ]
  [[ "$output" == *"usage: summarise-release-please.sh"* ]]
}

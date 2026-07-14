#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/validate-publish-inputs"
  SHA="0123456789abcdef0123456789abcdef01234567"
}

@test "accepts trusted repository dispatch without fallbacks" {
  run "${SCRIPT}" repository_dispatch zellij-plugin next "${SHA}" release/0.1 'github-actions[bot]' 'release-app[bot],github-actions[bot]'
  [ "$status" -eq 0 ]
  run jq -e --arg sha "${SHA}" '.source_sha == $sha and .source_branch == "release/0.1" and .channel == "next"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "rejects missing repository dispatch source identity" {
  run "${SCRIPT}" repository_dispatch zellij-plugin next '' main 'github-actions[bot]' 'github-actions[bot]'
  [ "$status" -ne 0 ]
  [[ "$output" == *"invalid source SHA"* ]]

  run "${SCRIPT}" repository_dispatch zellij-plugin next "${SHA}" '' 'github-actions[bot]' 'github-actions[bot]'
  [ "$status" -ne 0 ]
  [[ "$output" == *"untrusted source branch"* ]]
}

@test "matches dispatch actors exactly" {
  run "${SCRIPT}" repository_dispatch zellij-plugin next "${SHA}" main 'github-actionsb' 'github-actions[bot]'
  [ "$status" -ne 0 ]
  [[ "$output" == *"untrusted repository_dispatch actor"* ]]
}

@test "rejects obsolete tag and invalid channel" {
  run "${SCRIPT}" repository_dispatch zellij-plugin next "${SHA}" main 'github-actions[bot]' 'github-actions[bot]' next
  [ "$status" -ne 0 ]
  [[ "$output" == *"obsolete client_payload.tag"* ]]

  run "${SCRIPT}" workflow_dispatch zellij-plugin beta "${SHA}" main '' ''
  [ "$status" -ne 0 ]
  [[ "$output" == *"invalid channel"* ]]
}

@test "accepts manual dispatch without actor allowlist" {
  run "${SCRIPT}" workflow_dispatch docs latest "${SHA}" main '' ''
  [ "$status" -eq 0 ]
}

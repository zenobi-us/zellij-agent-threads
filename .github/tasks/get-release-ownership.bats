#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/get-release-ownership"
  REPO="$(mktemp -d)"
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"

  git -C "${REPO}" init -q
  git -C "${REPO}" config user.email test@example.com
  git -C "${REPO}" config user.name Test
  git -C "${REPO}" checkout -q -b main
  git -C "${REPO}" commit -q --allow-empty -m root
  ROOT="$(git -C "${REPO}" rev-parse HEAD)"
  git -C "${REPO}" update-ref refs/remotes/origin/main "${ROOT}"
  git -C "${REPO}" update-ref refs/remotes/origin/release/0.1 "${ROOT}"

  cat >"${MOCK_DIR}/gh" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${GH_PR_HEADS:-}"
EOF
  chmod +x "${MOCK_DIR}/gh" "${SCRIPT}"
}

teardown() {
  rm -rf "${REPO}" "${MOCK_DIR}"
}

run_ownership() {
  run bash -c 'cd "$1" && "$2" "$3" "$4" owner/repo' _ "${REPO}" "${SCRIPT}" "$1" "$2"
}

@test "release branch root is skipped" {
  run_ownership release/0.1 "${ROOT}"
  [ "$status" -eq 0 ]
  run jq -e '.process == false and .reason == "release-branch-root"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "post-root release commit is owned" {
  git -C "${REPO}" checkout -q -b release/0.1 "${ROOT}"
  git -C "${REPO}" commit -q --allow-empty -m hotfix
  HOTFIX="$(git -C "${REPO}" rev-parse HEAD)"
  git -C "${REPO}" update-ref refs/remotes/origin/release/0.1 "${HOTFIX}"

  run_ownership release/0.1 "${HOTFIX}"
  [ "$status" -eq 0 ]
  run jq -e '.process == true and .reason == "owned"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "associated release PR marks squash merge-back" {
  git -C "${REPO}" commit -q --allow-empty -m main-change
  COMMIT="$(git -C "${REPO}" rev-parse HEAD)"
  export GH_PR_HEADS=$'feature/nope\nrelease/0.1'

  run_ownership main "${COMMIT}"
  [ "$status" -eq 0 ]
  run jq -e '.process == false and .reason == "hotfix-merge-back"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "merge ancestry is fallback when PR lookup is empty" {
  git -C "${REPO}" checkout -q -b release/0.1 "${ROOT}"
  git -C "${REPO}" commit -q --allow-empty -m hotfix
  HOTFIX="$(git -C "${REPO}" rev-parse HEAD)"
  git -C "${REPO}" update-ref refs/remotes/origin/release/0.1 "${HOTFIX}"
  git -C "${REPO}" checkout -q main
  git -C "${REPO}" merge -q --no-ff release/0.1 -m merge-back
  MERGE="$(git -C "${REPO}" rev-parse HEAD)"

  run_ownership main "${MERGE}"
  [ "$status" -eq 0 ]
  run jq -e '.process == false and .reason == "hotfix-merge-back"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "normal main work remains owned" {
  git -C "${REPO}" commit -q --allow-empty -m normal
  COMMIT="$(git -C "${REPO}" rev-parse HEAD)"

  run_ownership main "${COMMIT}"
  [ "$status" -eq 0 ]
  run jq -e '.process == true and .reason == "owned"' <<<"$output"
  [ "$status" -eq 0 ]
}

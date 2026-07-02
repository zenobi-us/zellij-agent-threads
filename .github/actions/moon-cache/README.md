# Moon cache composite action

Caches only portable Moon artifacts for manual persistence in GitHub Actions:

- `.moon/cache/hashes`
- `.moon/cache/outputs`

Do **not** cache other paths in `.moon/cache`; they are not portable across machines.

## Why `cache-version` exists

Moon task hashes can change between CI runs. If cache contents drift, stale entries
can make restores ineffective. Bump `cache-version` to force invalidation.

## Usage

```yaml
- name: Restore Moon cache
  uses: ./.github/actions/moon-cache
  with:
    key-prefix: moon-pr
    cache-version: v1
```

Increase `cache-version` (for example `v2`) when cache quality degrades or after
major task/config/toolchain changes.

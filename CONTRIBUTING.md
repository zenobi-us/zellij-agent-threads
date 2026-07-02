# Contributing

## Workflow

1. Fork or branch from `main`.
2. Make a focused change.
3. Run checks:
   ```sh
   bun test
   bun run typecheck
   bun run build
   ```
4. Commit with Conventional Commits.
5. Open a pull request.

## Pull requests

- Keep PRs focused.
- PR titles should follow Conventional Commits.
- Include tests when behavior changes.

## Releases

Releases are managed by Release Please.

- Merge commits to `main` to create/update the release PR.
- Merge the release PR to publish GitHub release assets.
- The publish workflow builds the release binaries from `dist/` and uploads them to the GitHub release.

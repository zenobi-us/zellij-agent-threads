# Release Process

Boxfiles uses Release Please to manage version bumps and changelog PRs.

## Channels

- **Pre-release**: normal PRs to `main` create `x.x.x-next.J` release PRs.
- **Stable**: merged release PRs cut the final version.

## Publish

GitHub release assets are produced by the `publish.yml` workflow.

- release event: publish from the tagged commit
- manual run: choose a moon project id, usually `cli`

The publish task lives in the project `moon.yml`, so each app/pkg controls its own build or packaging steps.

## Notes

- Keep release metadata in `release-please-config.json` and `.release-please-manifest.json` in sync.
- Use conventional commits.

# Release process

This monorepo uses path-scoped automated semantic versioning.

## Artifact tag streams

Use artifact-prefixed tags only:

- `server-vX.Y.Z` for `server/**`
- `core-vX.Y.Z` for `core/**`
- `cli-vX.Y.Z` for `cli/**`
- `linux-app-vX.Y.Z` for `linux/**`

Future releasable app shells should use descriptive platform-specific streams such as `ios-app-vX.Y.Z`.

## Version selection

On pushes to `main`, `.github/workflows/release.yml` runs `scripts/semantic_release.py` once per artifact. For each artifact, it looks at commits touching that artifact's paths since the artifact's latest tag.

Conventional Commit rules apply per artifact path:

- `fix: ...` creates a patch release.
- `feat: ...` creates a minor release.
- `BREAKING CHANGE:` in the body or `feat!: ...` creates a major release.
- Other commit types do not create a release.

Prefer artifact scopes such as `fix(linux-app): ...` for changes under `linux/**`.

## Dry run locally

```sh
python3 scripts/semantic_release.py --artifact server --path server --dry-run
python3 scripts/semantic_release.py --artifact core --path core --dry-run
python3 scripts/semantic_release.py --artifact cli --path cli --dry-run
python3 scripts/semantic_release.py --artifact linux-app --path linux --dry-run
```

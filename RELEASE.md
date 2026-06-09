# Release process

This monorepo uses path-scoped automated semantic versioning.

## Artifacts

Each independently shipped artifact has its own tag stream:

- `server-vX.Y.Z` for `server/**`
- `core-vX.Y.Z` for `core/**`
- `app-vX.Y.Z` for platform app shells: `android/**`, `ios/**`, `linux/**`, `macos/**`, `windows/**`

## How versions are chosen

On every push to `main`, `.github/workflows/release.yml` runs `scripts/semantic_release.py` once per artifact. For each artifact, it looks at commits touching that artifact's paths since that artifact's latest tag.

Conventional Commit rules:

- `fix: ...` creates a patch release, e.g. `server-v1.2.3` -> `server-v1.2.4`
- `feat: ...` creates a minor release, e.g. `core-v1.2.3` -> `core-v1.3.0`
- `BREAKING CHANGE:` in the commit body, or `feat!: ...`, creates a major release, e.g. `app-v1.2.3` -> `app-v2.0.0`
- Other commit types do not create a release

## Outputs

For each artifact with releasable changes, the workflow creates:

- an annotated Git tag like `server-v1.2.3`
- a GitHub Release with generated notes for that artifact's path-scoped commits

## Commit examples

```text
fix(server): reject invalid blob nonces
feat(core): add recurrence parser
feat(app): add command palette
feat(server)!: change sync API response format
```

## Dry run locally

```sh
python3 scripts/semantic_release.py --artifact server --path server --dry-run
python3 scripts/semantic_release.py --artifact core --path core --dry-run
python3 scripts/semantic_release.py --artifact app --path "android ios linux macos windows" --dry-run
```

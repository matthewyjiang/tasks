# Release process

This monorepo uses path-scoped automated semantic versioning.

## Artifacts

Each independently shipped artifact has its own tag stream:

- `server-vX.Y.Z` for `server/**`
- `core-vX.Y.Z` for `core/**`
- `cli-vX.Y.Z` for `cli/**`
- `linux-app-vX.Y.Z` for the Linux GTK/libadwaita app in `linux/**`

Additional platform app shells should get descriptive platform-specific artifact streams when they become releasable, e.g. `macos-app-vX.Y.Z`, `windows-app-vX.Y.Z`, `android-app-vX.Y.Z`, and `ios-app-vX.Y.Z`.

## How versions are chosen

On every push to `main`, `.github/workflows/release.yml` runs `scripts/semantic_release.py` once per artifact. For each artifact, it looks at commits touching that artifact's paths since that artifact's latest tag.

Conventional Commit rules apply per artifact path; prefer artifact scopes such as `fix(linux-app): ...` for changes under `linux/**`.

- `fix: ...` creates a patch release, e.g. `server-v1.2.3` -> `server-v1.2.4`
- `feat: ...` creates a minor release, e.g. `core-v1.2.3` -> `core-v1.3.0`
- `BREAKING CHANGE:` in the commit body, or `feat!: ...`, creates a major release, e.g. `linux-app-v1.2.3` -> `linux-app-v2.0.0`
- Other commit types do not create a release

## Outputs

For each artifact with releasable changes, the workflow creates:

- an annotated Git tag like `server-v1.2.3`
- a GitHub Release with generated notes for that artifact's path-scoped commits

The release workflow does not push package-version commits directly to `main`, because repository rules require changes to land through pull requests. When package metadata needs a version bump, the workflow opens a release PR as `github-actions[bot]`; after that PR merges, the next release run creates the tag and GitHub Release. The release PR also prepends generated notes to the repository-level `CHANGELOG.md`. For `linux-app`, the release PR also keeps `packaging/arch/PKGBUILD`'s `pkgver` in sync with `linux/Cargo.toml`.

To regenerate `CHANGELOG.md` from existing artifact tags:

```sh
python3 scripts/semantic_release.py --backfill-changelog
```

## Commit examples

```text
fix(server): reject invalid blob nonces
feat(core): add recurrence parser
feat(cli): add sync status command
feat(linux-app): add command palette
docs(linux-app): document GTK validation before PRs
feat(server)!: change sync API response format
```

## Dry run locally

```sh
python3 scripts/semantic_release.py --artifact server --path server --dry-run
python3 scripts/semantic_release.py --artifact core --path core --dry-run
python3 scripts/semantic_release.py --artifact cli --path cli --dry-run
python3 scripts/semantic_release.py --artifact linux-app --path linux --dry-run
```

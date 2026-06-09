# AGENTS.md

## Semantic release naming rules

This repository uses path-scoped semantic versioning in `.github/workflows/release.yml` via `scripts/semantic_release.py`.

Tag names must use the artifact-prefixed format:

- `server-vX.Y.Z` for changes under `server/**`
- `core-vX.Y.Z` for changes under `core/**`
- `app-vX.Y.Z` for changes under platform app paths: `android/**`, `ios/**`, `linux/**`, `macos/**`, and `windows/**`

Do not create unscoped tags like `vX.Y.Z` for this monorepo. Each releasable artifact owns its own independent tag stream.

## Commit naming rules

Use Conventional Commit headers:

```text
<type>(<scope>): <description>
<type>(<scope>)!: <description>
```

Rules:

- Use lowercase commit types, for example `fix`, `feat`, `chore`, `docs`, `test`, or `refactor`.
- Prefer artifact scopes: `server`, `core`, or `app`.
- Keep descriptions imperative and concise, for example `fix(server): reject invalid blob nonces`.
- Use `!` for breaking changes, for example `feat(server)!: change sync response format`.
- If using a breaking-change footer, write `BREAKING CHANGE:` in the commit body.

Release-impacting types:

- `fix: ...` => patch
- `feat: ...` => minor
- `!` in the commit header or `BREAKING CHANGE:` in the body => major
- Other types do not create a release by themselves.

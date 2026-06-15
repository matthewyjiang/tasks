#!/usr/bin/env python3
"""Path-scoped semantic release helper for this monorepo.

Creates independent tags/releases per artifact, e.g. linux-app-v1.2.3 or server-v1.2.3, by analyzing
Conventional Commits that touched the artifact path since that artifact's latest tag.
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
import shlex
from pathlib import Path
from typing import Iterable


VERSION_RE = re.compile(r"^(?P<prefix>.+)-v(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)$")
HEADER_RE = re.compile(r"^(?P<type>[a-zA-Z]+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?: .+")
ARTIFACT_PATHS = {
    "server": ["server"],
    "core": ["core"],
    "cli": ["cli"],
    "linux-app": ["linux"],
}


@dataclass(frozen=True, order=True)
class Version:
    major: int
    minor: int
    patch: int

    def bump(self, level: str) -> "Version":
        if level == "major":
            return Version(self.major + 1, 0, 0)
        if level == "minor":
            return Version(self.major, self.minor + 1, 0)
        if level == "patch":
            return Version(self.major, self.minor, self.patch + 1)
        return self

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"


def run(args: list[str], *, check: bool = True) -> str:
    proc = subprocess.run(args, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if check and proc.returncode != 0:
        if proc.stdout:
            print(proc.stdout, file=sys.stdout, end="")
        if proc.stderr:
            print(proc.stderr, file=sys.stderr, end="")
        raise subprocess.CalledProcessError(proc.returncode, proc.args, output=proc.stdout, stderr=proc.stderr)
    return proc.stdout.strip()


def dispatch_linux_arch_package(tag: str) -> None:
    """Start Arch package publishing for auto-created linux-app tags.

    Tags pushed by the release workflow use GITHUB_TOKEN. GitHub suppresses most
    workflow runs caused by GITHUB_TOKEN-created events, including push tags, so
    the publish-arch-package workflow's tag trigger is only useful for manually
    pushed tags. workflow_dispatch is allowed, so dispatch it explicitly.
    """
    run(["gh", "workflow", "run", "publish-arch-package.yml", "--ref", tag])
    print(f"Dispatched publish-arch-package.yml for {tag}.")


def tags_for_artifact(prefix: str) -> list[tuple[str, Version]]:
    tags = run(["git", "tag", "--list", f"{prefix}-v*"], check=True).splitlines()
    parsed: list[tuple[str, Version]] = []
    for tag in tags:
        match = VERSION_RE.match(tag)
        if not match or match.group("prefix") != prefix:
            continue
        version = Version(int(match.group("major")), int(match.group("minor")), int(match.group("patch")))
        parsed.append((tag, version))
    return sorted(parsed, key=lambda item: item[1])


def latest_tag(prefix: str) -> tuple[str | None, Version]:
    tags = tags_for_artifact(prefix)
    if not tags:
        return None, Version(0, 0, 0)
    return tags[-1]


def normalize_paths(paths: Iterable[str]) -> list[str]:
    """Return pathspecs from one or more --path values.

    Git accepts path filtering as ``-- <path>...``. The workflow matrix may pass
    a space-separated value such as "android ios linux macos windows" as one
    shell argument, so split each value into individual pathspecs before
    forwarding them to git.
    """
    pathspecs: list[str] = []
    for path in paths:
        pathspecs.extend(shlex.split(path))
    return pathspecs


def commits_since(tag: str | None, paths: Iterable[str]) -> list[str]:
    rev = f"{tag}..HEAD" if tag else "HEAD"
    fmt = "%B%x1e"
    pathspecs = normalize_paths(paths)
    output = run(["git", "log", rev, f"--format={fmt}", "--", *pathspecs], check=True)
    if not output:
        return []
    return [entry.strip("\n") for entry in output.split("\x1e") if entry.strip()]


def bump_for_commit(message: str) -> str | None:
    lines = message.splitlines()
    header = lines[0] if lines else ""
    body = "\n".join(lines[1:])
    match = HEADER_RE.match(header)
    if "BREAKING CHANGE:" in body or "BREAKING-CHANGE:" in body:
        return "major"
    if not match:
        return None
    if match.group("breaking"):
        return "major"
    typ = match.group("type").lower()
    if typ == "feat":
        return "minor"
    if typ == "fix":
        return "patch"
    return None


def highest_bump(commits: Iterable[str]) -> str | None:
    order = {None: 0, "patch": 1, "minor": 2, "major": 3}
    best: str | None = None
    for commit in commits:
        bump = bump_for_commit(commit)
        if order[bump] > order[best]:
            best = bump
    return best


def release_notes_for_range(tag: str, previous_tag: str | None, end_ref: str, paths: Iterable[str]) -> str:
    rev = f"{previous_tag}..{end_ref}" if previous_tag else end_ref
    pathspecs = normalize_paths(paths)
    artifact = tag.rsplit("-v", 1)[0]
    output = run(["git", "log", rev, "--pretty=format:%s%x1f%h%x1e", "--", *pathspecs], check=True)
    lines: list[str] = []
    for entry in output.split("\x1e"):
        if not entry.strip():
            continue
        subject, short_hash = entry.strip().split("\x1f", 1)
        if subject.startswith(f"chore({artifact}): release {tag}"):
            continue
        lines.append(f"- {subject} ({short_hash})")
    return f"## {tag}\n\n" + ("\n".join(lines) or "No user-facing changes.") + "\n"


def release_notes(tag: str, previous_tag: str | None, paths: Iterable[str]) -> str:
    return release_notes_for_range(tag, previous_tag, "HEAD", paths)


def tag_created(tag: str) -> tuple[int, str]:
    output = run(
        ["git", "for-each-ref", f"refs/tags/{tag}", "--format=%(creatordate:unix)%09%(creatordate:short)"],
        check=True,
    )
    timestamp, date = output.split("\t", 1)
    return int(timestamp), date


def dated_release_notes(tag: str, previous_tag: str | None, end_ref: str, paths: Iterable[str], date: str) -> str:
    notes = release_notes_for_range(tag, previous_tag, end_ref, paths).rstrip()
    return notes.replace(f"## {tag}", f"## {tag} - {date}", 1)


def update_changelog(changelog: Path, tag: str, previous_tag: str | None, paths: Iterable[str]) -> bool:
    """Prepend this artifact release's generated notes to a repository changelog."""
    existing = changelog.read_text() if changelog.exists() else "# Changelog\n"
    if re.search(rf"^## {re.escape(tag)}(?:\s|$)", existing, re.MULTILINE):
        return False

    dated_notes = dated_release_notes(tag, previous_tag, "HEAD", paths, dt.date.today().isoformat())
    if existing.startswith("# Changelog\n"):
        updated = existing.replace("# Changelog\n", f"# Changelog\n\n{dated_notes}\n", 1)
    else:
        updated = f"# Changelog\n\n{dated_notes}\n\n{existing}"
    changelog.write_text(updated if updated.endswith("\n") else f"{updated}\n")
    return True


def backfill_changelog(changelog: Path, artifacts: Iterable[str]) -> None:
    entries: list[tuple[int, str]] = []
    for artifact in artifacts:
        paths = ARTIFACT_PATHS[artifact]
        previous_tag: str | None = None
        for tag, _version in tags_for_artifact(artifact):
            timestamp, date = tag_created(tag)
            entries.append((timestamp, dated_release_notes(tag, previous_tag, tag, paths, date)))
            previous_tag = tag

    entries.sort(key=lambda item: item[0], reverse=True)
    content = "# Changelog\n\n" + "\n\n".join(notes for _timestamp, notes in entries) + "\n"
    changelog.write_text(content)


def replace_package_version(manifest: Path, version: Version) -> None:
    text = manifest.read_text()
    lines = text.splitlines(keepends=True)
    in_package = False
    for idx, line in enumerate(lines):
        stripped = line.strip()
        if stripped == "[package]":
            in_package = True
            continue
        if in_package and stripped.startswith("["):
            break
        if in_package and stripped.startswith("version = "):
            newline = "\n" if line.endswith("\n") else ""
            lines[idx] = f'version = "{version}"{newline}'
            manifest.write_text("".join(lines))
            return
    raise RuntimeError(f"did not find [package] version in {manifest}")


def replace_arch_pkgbuild_version(pkgbuild: Path, version: Version) -> None:
    text = pkgbuild.read_text()
    lines = text.splitlines(keepends=True)
    for idx, line in enumerate(lines):
        if line.startswith("pkgver="):
            newline = "\n" if line.endswith("\n") else ""
            lines[idx] = f"pkgver={version}{newline}"
            pkgbuild.write_text("".join(lines))
            return
    raise RuntimeError(f"did not find pkgver in {pkgbuild}")


def update_cargo_lock_package(lockfile: Path, package: str, version: Version) -> None:
    if not lockfile.exists():
        return
    text = lockfile.read_text()
    blocks = text.split("\n[[package]]\n")
    changed = False
    for idx, block in enumerate(blocks):
        if f'name = "{package}"' not in block:
            continue
        lines = block.splitlines(keepends=True)
        for line_idx, line in enumerate(lines):
            if line.startswith("version = "):
                newline = "\n" if line.endswith("\n") else ""
                lines[line_idx] = f'version = "{version}"{newline}'
                blocks[idx] = "".join(lines)
                changed = True
                break
    if changed:
        lockfile.write_text("\n[[package]]\n".join(blocks))


def update_artifact_version(artifact: str, version: Version) -> list[str]:
    if os.environ.get("RELEASE_UPDATE_PACKAGE_VERSION") != "1":
        return []
    if artifact == "cli":
        replace_package_version(Path("cli/Cargo.toml"), version)
        update_cargo_lock_package(Path("Cargo.lock"), "taskmanager-cli", version)
        return ["cli/Cargo.toml", "Cargo.lock"]
    if artifact == "core":
        replace_package_version(Path("core/Cargo.toml"), version)
        update_cargo_lock_package(Path("Cargo.lock"), "taskmanager-core", version)
        return ["core/Cargo.toml", "Cargo.lock"]
    if artifact == "linux-app":
        replace_package_version(Path("linux/Cargo.toml"), version)
        update_cargo_lock_package(Path("Cargo.lock"), "tsk-linux", version)
        replace_arch_pkgbuild_version(Path("packaging/arch/PKGBUILD"), version)
        return ["linux/Cargo.toml", "Cargo.lock", "packaging/arch/PKGBUILD"]
    return []


def has_staged_changes() -> bool:
    proc = subprocess.run(["git", "diff", "--cached", "--quiet"], check=False)
    return proc.returncode != 0


def create_version_pr(artifact: str, tag: str, base_branch: str) -> None:
    branch = f"release/{tag}"
    run(["git", "branch", "-M", branch])
    run(["git", "push", "--force-with-lease", "-u", "origin", branch])

    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print(f"GITHUB_TOKEN not set; pushed {branch} but skipped PR creation.")
        return

    existing = run(
        [
            "gh",
            "pr",
            "list",
            "--head",
            branch,
            "--base",
            base_branch,
            "--state",
            "open",
            "--json",
            "number",
            "--jq",
            ".[0].number // empty",
        ]
    )
    if existing:
        print(f"Version bump PR already exists: #{existing}")
        return

    with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as body_file:
        body_file.write(
            f"Update package metadata and changelog for `{tag}`. "
            "The release workflow will create the tag after this PR merges.\n"
        )
        body_path = body_file.name
    try:
        run(
            [
                "gh",
                "pr",
                "create",
                "--base",
                base_branch,
                "--head",
                branch,
                "--title",
                f"chore({artifact}): release {tag}",
                "--body-file",
                body_path,
            ]
        )
    finally:
        Path(body_path).unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", help="Artifact name used as tag prefix, e.g. server")
    parser.add_argument("--path", nargs="+", help="Path(s) to inspect for changes, e.g. server or android ios")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--backfill-changelog",
        action="store_true",
        help="Regenerate CHANGELOG.md from all existing artifact tags and exit.",
    )
    args = parser.parse_args()

    if args.backfill_changelog:
        artifacts = [args.artifact] if args.artifact else ARTIFACT_PATHS.keys()
        unknown = [artifact for artifact in artifacts if artifact not in ARTIFACT_PATHS]
        if unknown:
            parser.error(f"unknown artifact(s) for changelog backfill: {', '.join(unknown)}")
        if args.dry_run:
            print("Would regenerate CHANGELOG.md for: " + ", ".join(artifacts))
            return 0
        backfill_changelog(Path("CHANGELOG.md"), artifacts)
        print("Regenerated CHANGELOG.md.")
        return 0

    if not args.artifact or not args.path:
        parser.error("--artifact and --path are required unless --backfill-changelog is used")

    previous_tag, current = latest_tag(args.artifact)
    commits = commits_since(previous_tag, args.path)
    bump = highest_bump(commits)
    if bump is None:
        print(f"No releasable {args.artifact} changes.")
        return 0

    next_version = current.bump(bump)
    tag = f"{args.artifact}-v{next_version}"
    notes = release_notes(tag, previous_tag, args.path)
    print(f"{args.artifact}: {current} -> {next_version} ({bump})")

    if args.dry_run:
        print(notes)
        return 0

    updated_files = update_artifact_version(args.artifact, next_version)
    if os.environ.get("RELEASE_UPDATE_CHANGELOG") == "1":
        if update_changelog(Path("CHANGELOG.md"), tag, previous_tag, args.path):
            updated_files.append("CHANGELOG.md")
    if updated_files:
        run(["git", "add", *updated_files])
        if has_staged_changes():
            run(
                [
                    "git",
                    "commit",
                    "-m",
                    f"chore({args.artifact}): release {tag}",
                    "-m",
                    f"Update package metadata to {tag}.",
                ]
            )
            branch = os.environ.get("GITHUB_REF_NAME", "main")
            if os.environ.get("RELEASE_VERSION_PR") == "1":
                create_version_pr(args.artifact, tag, branch)
                print(f"Opened version bump PR for {tag}; release will be created after it merges.")
                return 0

            push = subprocess.run(
                ["git", "push", "origin", f"HEAD:{branch}"],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            if push.returncode != 0:
                if push.stdout:
                    print(push.stdout, file=sys.stdout, end="")
                if push.stderr:
                    print(push.stderr, file=sys.stderr, end="")
                run(["git", "fetch", "origin", branch])
                run(["git", "rebase", f"origin/{branch}"])
                run(["git", "push", "origin", f"HEAD:{branch}"])
            notes = release_notes(tag, previous_tag, args.path)

    # Equivalent shell command: git tag -a <tag> -m "Release <tag>"
    run(["git", "tag", "-a", tag, "-m", f"Release {tag}"])
    run(["git", "push", "origin", tag])

    token = os.environ.get("GITHUB_TOKEN")
    if token:
        run(["gh", "release", "create", tag, "--title", tag, "--notes", notes])
        if args.artifact == "linux-app":
            dispatch_linux_arch_package(tag)
    else:
        print("GITHUB_TOKEN not set; skipped GitHub Release creation.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

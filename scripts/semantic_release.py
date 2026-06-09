#!/usr/bin/env python3
"""Path-scoped semantic release helper for this monorepo.

Creates independent tags/releases per artifact, e.g. artifact-v1.2.3 or server-v1.2.3, by analyzing
Conventional Commits that touched the artifact path since that artifact's latest tag.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from dataclasses import dataclass
import shlex
from typing import Iterable


VERSION_RE = re.compile(r"^(?P<prefix>.+)-v(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)$")
HEADER_RE = re.compile(r"^(?P<type>[a-zA-Z]+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?: .+")


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
    proc = subprocess.run(args, check=check, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return proc.stdout.strip()


def latest_tag(prefix: str) -> tuple[str | None, Version]:
    tags = run(["git", "tag", "--list", f"{prefix}-v*"], check=True).splitlines()
    best_tag: str | None = None
    best = Version(0, 0, 0)
    for tag in tags:
        match = VERSION_RE.match(tag)
        if not match or match.group("prefix") != prefix:
            continue
        version = Version(int(match.group("major")), int(match.group("minor")), int(match.group("patch")))
        if best_tag is None or version > best:
            best_tag, best = tag, version
    return best_tag, best


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


def release_notes(tag: str, previous_tag: str | None, paths: Iterable[str]) -> str:
    rev = f"{previous_tag}..HEAD" if previous_tag else "HEAD"
    pathspecs = normalize_paths(paths)
    log = run(["git", "log", rev, "--pretty=format:- %s (%h)", "--", *pathspecs], check=True)
    return f"## {tag}\n\n" + (log or "No user-facing changes.") + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", required=True, help="Artifact name used as tag prefix, e.g. server")
    parser.add_argument("--path", required=True, nargs="+", help="Path(s) to inspect for changes, e.g. server or android ios")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

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

    # Equivalent shell command: git tag -a <tag> -m "Release <tag>"
    run(["git", "tag", "-a", tag, "-m", f"Release {tag}"])
    run(["git", "push", "origin", tag])

    token = os.environ.get("GITHUB_TOKEN")
    if token:
        run(["gh", "release", "create", tag, "--title", tag, "--notes", notes])
    else:
        print("GITHUB_TOKEN not set; skipped GitHub Release creation.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

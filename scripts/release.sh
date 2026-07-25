#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./scripts/release.sh [major|minor|patch]

Bump the package version (default: minor), validate the crate, create a release
commit and annotated tag, then publish to crates.io. The script never pushes.
EOF
}

confirm() {
    local prompt="$1"
    local answer

    [[ -t 0 ]] || return 1
    read -r -p "$prompt [y/N] " answer || return 1
    [[ "$answer" =~ ^[Yy]([Ee][Ss])?$ ]]
}

case "${1:-minor}" in
    -h|--help)
        usage
        exit 0
        ;;
    major|minor|patch)
        bump="${1:-minor}"
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

if (( $# > 1 )); then
    usage >&2
    exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cd "$repo_root"

for command in git cargo python3; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf 'error: required command not found: %s\n' "$command" >&2
        exit 1
    fi
done

if [[ -n "$(git status --porcelain)" ]]; then
    printf 'error: Git working tree must be clean\n' >&2
    exit 1
fi

if ! branch="$(git symbolic-ref --quiet --short HEAD)"; then
    printf 'error: cannot release from detached HEAD\n' >&2
    exit 1
fi

cargo_home="${CARGO_HOME:-$HOME/.cargo}"
credentials_configured=false
if [[ -n "${CARGO_REGISTRY_TOKEN:-}" || -n "${CARGO_REGISTRIES_CRATES_IO_TOKEN:-}" ]]; then
    credentials_configured=true
else
    for credentials_file in "$cargo_home/credentials.toml" "$cargo_home/credentials"; do
        if [[ -f "$credentials_file" ]] && python3 - "$credentials_file" <<'PY'
import pathlib
import re
import sys

section = None
for line in pathlib.Path(sys.argv[1]).read_text().splitlines():
    section_match = re.match(r'^\s*\[([^]]+)]\s*(?:#.*)?$', line)
    if section_match:
        section = section_match.group(1).strip()
        continue
    if section in {"registry", "registries.crates-io"}:
        token_match = re.match(r'^\s*token\s*=\s*(.+?)\s*$', line)
        if token_match and re.fullmatch(r'''(?:"[^"]+"|'[^']+')''', token_match.group(1)):
            raise SystemExit(0)
raise SystemExit(1)
PY
        then
            credentials_configured=true
            break
        fi
    done
fi

if [[ "$credentials_configured" != true ]]; then
    printf 'error: crates.io credentials were not detected; run `cargo login` before retrying\n' >&2
    exit 1
fi

read -r old_version new_version < <(python3 - "$bump" <<'PY'
import pathlib
import re
import sys

text = pathlib.Path("Cargo.toml").read_text()
package = re.search(r'(?ms)^\[package\]\s*$\n(?P<body>.*?)(?=^\[|\Z)', text)
if package is None:
    raise SystemExit("error: Cargo.toml has no [package] section")
match = re.search(r'(?m)^\s*version\s*=\s*["\']([^"\']+)["\']', package.group("body"))
if match is None:
    raise SystemExit("error: [package] has no version")
old = match.group(1)
parts = old.split(".")
if len(parts) != 3 or not all(part.isdigit() for part in parts):
    raise SystemExit(f"error: package version must be X.Y.Z, found {old!r}")
major, minor, patch = map(int, parts)
bump = sys.argv[1]
if bump == "major":
    new = (major + 1, 0, 0)
elif bump == "minor":
    new = (major, minor + 1, 0)
else:
    new = (major, minor, patch + 1)
print(old, ".".join(map(str, new)))
PY
)

tag="v$new_version"
if git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null; then
    printf 'error: tag %s already exists\n' "$tag" >&2
    exit 1
fi

python3 - "$old_version" "$new_version" <<'PY'
import pathlib
import re
import sys

old, new = sys.argv[1:]
manifest_path = pathlib.Path("Cargo.toml")
manifest = manifest_path.read_text()
package = re.search(r'(?ms)^\[package\]\s*$\n(?P<body>.*?)(?=^\[|\Z)', manifest)
body = package.group("body")
updated_body, manifest_count = re.subn(
    r'(?m)^(\s*version\s*=\s*["\'])' + re.escape(old) + r'(["\'])',
    rf'\g<1>{new}\g<2>',
    body,
    count=1,
)
if manifest_count != 1:
    raise SystemExit("error: package version changed unexpectedly")
updated_manifest = manifest[:package.start("body")] + updated_body + manifest[package.end("body"):]

lock_path = pathlib.Path("Cargo.lock")
lock = lock_path.read_text()
blocks = list(re.finditer(r'(?ms)^\[\[package\]\]\s*$\n.*?(?=^\[\[package\]\]|\Z)', lock))
matches = []
for block in blocks:
    name = re.search(r'(?m)^name\s*=\s*["\']([^"\']+)["\']', block.group())
    version = re.search(r'(?m)^version\s*=\s*["\']([^"\']+)["\']', block.group())
    if name and version and name.group(1) == "tuicore" and version.group(1) == old:
        matches.append((block, version))
if len(matches) != 1:
    raise SystemExit(f"error: expected exactly one tuicore {old} package in Cargo.lock, found {len(matches)}")
block, version = matches[0]
version_start = block.start() + version.start(1)
version_end = block.start() + version.end(1)
updated_lock = lock[:version_start] + new + lock[version_end:]

manifest_path.write_text(updated_manifest)
lock_path.write_text(updated_lock)
PY

cargo test

printf '\nVersion: %s -> %s\n' "$old_version" "$new_version"
git --no-pager diff -- Cargo.toml Cargo.lock
printf 'Next: create commit %q, then validate the clean crates.io package.\n' "release: $tag"
if ! confirm "Commit, tag, and publish $tag to crates.io?"; then
    printf 'Release canceled; version changes remain in working tree.\n' >&2
    exit 1
fi

git add Cargo.toml Cargo.lock
git commit -m "release: $tag"

if ! cargo package --registry crates-io || ! cargo publish --dry-run --registry crates-io; then
    printf '\nValidation failed. Local release commit remains; tag %s was not created.\n' "$tag" >&2
    printf 'Inspect with: git show --stat HEAD && git status\n' >&2
    printf 'Resume validation: cargo package --registry crates-io && cargo publish --dry-run --registry crates-io\n' >&2
    printf 'After both pass, inspect the commit and create the tag/publish deliberately; do not rerun this bump.\n' >&2
    exit 1
fi

git tag -a "$tag" -m "release: $tag"

if [[ -n "$(git status --porcelain)" ]]; then
    printf 'error: Git working tree must be clean before publishing\n' >&2
    exit 1
fi
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse "$tag^{commit}")" ]]; then
    printf 'error: HEAD must match release tag %s before publishing\n' "$tag" >&2
    exit 1
fi

if ! cargo publish --registry crates-io; then
    printf '\nPublish failed. Local release commit and tag %s remain.\n' "$tag" >&2
    printf 'Inspect with: git show --stat %s && git status\n' "$tag" >&2
    exit 1
fi

printf '\nPublished %s. Push release commit and tag when ready:\n' "$tag"
printf 'git push origin %s\n' "$branch"
printf 'git push origin %s\n' "$tag"

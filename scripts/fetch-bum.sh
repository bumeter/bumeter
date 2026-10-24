#!/usr/bin/env bash
# Downloads bum $(cat BUM_VERSION) for one Rust target from bumeter/statusline's release, checks it
# against the release's SHA256SUMS and writes it to <out>.
# Usage: scripts/fetch-bum.sh <target-triple> <out>
set -euo pipefail
target="$1"; out="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(tr -d ' \r\n' < "$root/BUM_VERSION")"
ext=""; case "$target" in *windows*) ext=".exe" ;; esac
name="bum-$target$ext"
base="https://github.com/bumeter/statusline/releases/download/v$version"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
curl -fsSL --retry 3 -o "$tmp/$name" "$base/$name"
curl -fsSL --retry 3 -o "$tmp/SHA256SUMS" "$base/SHA256SUMS"
line="$(grep " $name\$" "$tmp/SHA256SUMS")" || { echo "$name is not in SHA256SUMS of v$version" >&2; exit 1; }
if command -v sha256sum >/dev/null; then (cd "$tmp" && echo "$line" | sha256sum -c -)
else (cd "$tmp" && echo "$line" | shasum -a 256 -c -); fi
mkdir -p "$(dirname "$out")"
mv "$tmp/$name" "$out"
chmod +x "$out"
echo "bum $version for $target → $out"

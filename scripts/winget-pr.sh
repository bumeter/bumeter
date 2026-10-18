#!/usr/bin/env bash
# Open the WinGet pull request for one release, from the manifests in packaging/winget/.
#
#   scripts/winget-pr.sh --version 1.0.5            # fork defaults to chieund (WINGET_FORK overrides)
#   scripts/winget-pr.sh --version 1.0.5 --no-push  # stop after the commit and print the push command
#   scripts/winget-pr.sh --fork someone --dir /tmp/winget-pkgs   # reuse an existing clone
#
# Winget packages live in one monorepo, so a release is a branch plus a pull request. The three files
# come from scripts/packaging.py. This clones your fork sparsely, copies them into
# manifests/b/Bumeter/Bumeter/<version>/, checks them against the official schema, commits with the
# message winget-pkgs expects, pushes, and prints the URL to open the pull request (or opens it with
# `gh` when that is installed).
#
# Forking is a web click the first time: https://github.com/microsoft/winget-pkgs/fork
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/packaging/winget"
VERSION=""
FORK="${WINGET_FORK:-chieund}"
REPO="${WINGET_REPO:-$FORK/bumeter}"
DIR=""
PUSH=1

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
    --fork) FORK="${2:?--fork needs a value}"; shift 2 ;;
    --dir) DIR="${2:?--dir needs a value}"; shift 2 ;;
    --no-push) PUSH=0; shift ;;
    -h|--help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[ -f "$SRC/Bumeter.Bumeter.yaml" ] || { echo "no manifests in $SRC — run scripts/packaging.py first" >&2; exit 1; }
# `| tr -d '\r'`: the working tree can be CRLF (core.autocrlf), and a stray CR here turns every
# name into "Bumeter.Bumeter\r.yaml" — a file that does not exist.
VERSION="${VERSION:-$(sed -n 's/^PackageVersion: *//p' "$SRC/Bumeter.Bumeter.yaml" | tr -d '\r')}"
ID="$(sed -n 's/^PackageIdentifier: *//p' "$SRC/Bumeter.Bumeter.yaml" | tr -d '\r')"
MV="$(sed -n 's/^ManifestVersion: *//p' "$SRC/Bumeter.Bumeter.yaml" | tr -d '\r')"
PUBLISHER="${ID%%.*}"
NAME="${ID##*.}"
FIRST="$(printf '%s' "$PUBLISHER" | cut -c1 | tr '[:upper:]' '[:lower:]')"
DEST="manifests/$FIRST/$PUBLISHER/$NAME/$VERSION"
BRANCH="bumeter-$VERSION"
FILES=("$ID.yaml" "$ID.installer.yaml" "$ID.locale.en-US.yaml")
for f in "${FILES[@]}"; do
  [ -f "$SRC/$f" ] || { echo "missing $SRC/$f — run scripts/packaging.py" >&2; exit 1; }
done

FORK_URL="https://github.com/$FORK/winget-pkgs.git"
if ! GIT_TERMINAL_PROMPT=0 git ls-remote --exit-code "$FORK_URL" HEAD >/dev/null 2>&1; then
  echo "no repository at $FORK_URL" >&2
  echo "Fork winget-pkgs first (one click): https://github.com/microsoft/winget-pkgs/fork" >&2
  exit 1
fi

DIR="${DIR:-$PWD/target/winget-pkgs}"
if [ ! -d "$DIR/.git" ]; then
  echo "cloning $FORK/winget-pkgs (shallow, only manifests/$FIRST/$PUBLISHER)…" >&2
  # A blobless partial clone plus a sparse checkout: a few hundred KB, not the whole catalogue.
  git clone --depth 1 --filter=blob:none --sparse "$FORK_URL" "$DIR" >/dev/null
  git -C "$DIR" sparse-checkout set -q "manifests/$FIRST/$PUBLISHER"
else
  git -C "$DIR" fetch -q origin
fi
git -C "$DIR" checkout -q -B "$BRANCH" origin/master 2>/dev/null || git -C "$DIR" checkout -q -B "$BRANCH"

# One version already in the catalogue means this is an update, and the pull request title says which
# ("New package: X version 1.0.5" / "Update: X to 1.0.6" — the template's own wording).
EXISTING="$(git -C "$DIR" ls-tree --name-only "origin/master:manifests/$FIRST/$PUBLISHER/$NAME" 2>/dev/null || true)"
if [ -n "$EXISTING" ]; then
  TITLE="Update: $ID to $VERSION"
else
  TITLE="New package: $ID version $VERSION"
fi

mkdir -p "$DIR/$DEST"
for f in "${FILES[@]}"; do
  sed 's/\r$//' "$SRC/$f" > "$DIR/$DEST/$f"   # LF in the pull request, whatever the checkout says
  head -1 "$DIR/$DEST/$f" | grep -q yaml-language-server \
    || echo "! $f has no yaml-language-server header, which the winget-pkgs checklist asks for" >&2
done

# The schema is what the winget-pkgs bot checks first; catching it here saves a round trip.
python3 - "$DIR/$DEST" <<'PY' || exit 1
import json, pathlib, sys, tempfile, urllib.request

try:
    import jsonschema, yaml
except ImportError:
    print("! jsonschema/PyYAML not installed: skipping the schema check (pip install jsonschema pyyaml)", file=sys.stderr)
    sys.exit(0)

KIND = {"version": "version", "installer": "installer", "defaultLocale": "defaultLocale"}
cache = pathlib.Path(tempfile.gettempdir())
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.yaml")):
    text = path.read_text(encoding="utf-8")
    data = yaml.safe_load(text)
    kind = KIND[data["ManifestType"]]
    version = data["ManifestVersion"]
    url = f"https://aka.ms/winget-manifest.{kind}.{version}.schema.json"
    local = cache / f"winget-{kind}-{version}.schema.json"
    if not local.exists():
        local.write_bytes(urllib.request.urlopen(url, timeout=30).read())
    schema = json.loads(local.read_text(encoding="utf-8"))
    jsonschema.validate(data, schema)
    print(f"ok  {path.name}  ({data['ManifestType']} {version})")
PY

git -C "$DIR" add "$DEST"
# winget-pkgs matches the commit author to a GitHub account for the CLA, so a placeholder email stops
# the pull request before a human ever looks at it.
[ -n "$(git config --get user.email || true)" ] || { echo "git has no user.email set — winget-pkgs needs it to match your GitHub account" >&2; exit 1; }
echo "committing as $(git config --get user.name) <$(git config --get user.email)>" >&2
git -C "$DIR" commit -q -m "$TITLE" || echo "nothing committed — $BRANCH already carries $VERSION" >&2

BODY="$(cat <<EOF
$TITLE

Bumeter is a local-only desktop app that shows where AI coding tokens go: which conversations and
turns spent the most, which skills load into every session, and how much of the 5-hour, 7-day and
monthly limits is left. Claude Code, Kiro and OpenCode; nothing is uploaded.

- Installer: the NSIS \`.exe\` Tauri builds (\`InstallerType: nullsoft\`, per-user install, no admin prompt)
- \`InstallerSha256\` taken from the release asset, \`Scope: user\`
- Release page: https://github.com/$REPO/releases/tag/v$VERSION
- Manifests checked against the official winget $MV schemas locally (\`winget validate\` needs Windows)
- The installer is not code-signed yet, so SmartScreen warns on the first manual run
EOF
)"

if [ "$PUSH" = 0 ]; then
  echo
  echo "commit ready in $DIR on $BRANCH (nothing pushed)"
  echo "push with: git -C $DIR push -u origin $BRANCH"
else
  git -C "$DIR" push -q -u origin "$BRANCH"
  echo "pushed $BRANCH to $FORK/winget-pkgs" >&2
  if command -v gh >/dev/null 2>&1; then
    gh pr create --repo microsoft/winget-pkgs --base master --head "$FORK:$BRANCH" --title "$TITLE" --body "$BODY"
  else
    echo
    echo "open the pull request here:"
    echo "  https://github.com/microsoft/winget-pkgs/compare/master...$FORK:$BRANCH?expand=1"
  fi
fi

echo
echo "title: $TITLE"
echo "----- body -----"
echo "$BODY"

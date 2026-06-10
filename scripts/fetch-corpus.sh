#!/usr/bin/env bash
# fetch-corpus.sh — materialise external corpus programs into corpus/programs/
#
# Reads corpus/manifest.toml and, for each [[program]] entry, performs a
# blobless sparse clone at the pinned commit SHA.  The resulting trees land in
# corpus/programs/<name>/ which is gitignored — the source is never committed.
#
# Usage
# -----
#   scripts/fetch-corpus.sh           # skip entries that already exist
#   scripts/fetch-corpus.sh --force   # re-clone even if already present
#
# Requirements: git >=2.36 (for --filter=blob:none + sparse-checkout),
#               python3 >=3.11 (for tomllib, which is stdlib since 3.11).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$REPO_ROOT/corpus/manifest.toml"
PROGRAMS_DIR="$REPO_ROOT/corpus/programs"
FORCE=0

for arg in "$@"; do
    case "$arg" in
        --force) FORCE=1 ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

if [[ ! -f "$MANIFEST" ]]; then
    echo "ERROR: manifest not found at $MANIFEST" >&2
    exit 1
fi

# Parse the manifest with Python's stdlib tomllib (Python 3.11+).
# Outputs lines of the form: NAME|REPO|SHA|SUBPATH
parse_manifest() {
    python3 - "$MANIFEST" <<'PYEOF'
import sys, tomllib, pathlib

manifest_path = pathlib.Path(sys.argv[1])
with open(manifest_path, "rb") as fh:
    data = tomllib.load(fh)

for prog in data.get("program", []):
    name    = prog["name"]
    repo    = prog["repo"]
    sha     = prog["sha"]
    subpath = prog.get("subpath", "")
    # Use pipe as separator; none of the fields contain pipes.
    print(f"{name}|{repo}|{sha}|{subpath}")
PYEOF
}

mkdir -p "$PROGRAMS_DIR"

echo "==> Fetching external corpus programs into $PROGRAMS_DIR"

while IFS='|' read -r NAME REPO SHA SUBPATH; do
    DEST="$PROGRAMS_DIR/$NAME"

    if [[ -d "$DEST" && "$FORCE" -eq 0 ]]; then
        echo "  [skip] $NAME — already present (use --force to re-clone)"
        continue
    fi

    if [[ -d "$DEST" && "$FORCE" -eq 1 ]]; then
        echo "  [force] removing existing $DEST"
        rm -rf "$DEST"
    fi

    echo "  [fetch] $NAME @ ${SHA:0:12} from $REPO"

    # Blobless clone: avoids downloading blob objects up front (fast, shallow on
    # content while keeping full commit history for the sparse checkout below).
    git clone \
        --filter=blob:none \
        --no-checkout \
        --quiet \
        "$REPO" \
        "$DEST"

    pushd "$DEST" > /dev/null

    if [[ -n "$SUBPATH" ]]; then
        # Enable sparse checkout restricted to the requested sub-path so we only
        # materialise the relevant program source.
        git sparse-checkout init --cone
        git sparse-checkout set "$SUBPATH"
    fi

    git checkout --quiet "$SHA"

    popd > /dev/null

    echo "  [ok]    $NAME"
done < <(parse_manifest)

echo "==> Done."

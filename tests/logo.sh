#!/bin/sh
# Regenerate the terminal logo grid in crates/forgeguard-cli/src/main.rs from
# assets/brand/logo-mark.svg, so the banner and the real mark cannot drift apart.
#
#   sh tests/logo.sh          print the grid
#   sh tests/logo.sh --check  fail if the committed grid differs from the SVG
#
# Needs macOS `qlmanage` and Python with Pillow — both developer-only, which is
# why this is a helper rather than a build step. The grid is committed, so
# building ForgeGuard never needs either.
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT HUP INT TERM

command -v qlmanage >/dev/null 2>&1 || { echo "skip: qlmanage is macOS only" >&2; exit 0; }
python3 -c 'import PIL' 2>/dev/null || { echo "skip: Pillow is not installed" >&2; exit 0; }

# The shield and hammer only, over a chroma key that marks everything outside
# the mark. The card background and rounded corners are dropped: the banner sits
# on whatever background the terminal already has.
sed -e 's|<rect width="32" height="32" rx="7" fill="#0a0a0a"/>|<rect width="32" height="32" fill="#ff00ff"/>|' \
    -e 's|stroke="#0a0a0a"|stroke="#000000"|' \
    "${repository_root}/assets/brand/logo-mark.svg" > "${work}/key.svg"

qlmanage -t -s 512 -o "$work" "${work}/key.svg" >/dev/null 2>&1
test -f "${work}/key.svg.png" || { echo "error: qlmanage produced no PNG" >&2; exit 1; }

generate() {
python3 - "$work" <<'PY'
import sys
from PIL import Image

work = sys.argv[1]
image = Image.open(f"{work}/key.svg.png").convert("RGB")


def classify(pixel):
    """Chroma key and QuickLook's white frame are transparent; the rest is the mark."""
    red, green, blue = pixel
    if (red > 150 and blue > 150 and green < 110) or red + green + blue > 600:
        return " "
    if green > red + 25 and green > blue + 25:
        return "g"
    if red + green + blue < 220:
        return "d"
    return "g"


pixels = image.load()
width, height = image.size
columns = [x for x in range(width) if any(classify(pixels[x, y]) != " " for y in range(height))]
rows = [y for y in range(height) if any(classify(pixels[x, y]) != " " for x in range(width))]
mark = image.crop((min(columns), min(rows), max(columns) + 1, max(rows) + 1))

# Twelve columns by twelve pixel rows renders as twelve columns by six text
# rows, the smallest size at which the hammer inside the shield stays legible.
scaled = mark.resize((12, 12), Image.LANCZOS).load()
for y in range(12):
    print('    "' + "".join(classify(scaled[x, y]) for x in range(12)) + '",')
PY
}

source_file="${repository_root}/crates/forgeguard-cli/src/main.rs"

if [ "${1:-}" = "--check" ]; then
    generate > "${work}/expected"
    # The committed grid is the twelve quoted rows following `const LOGO`.
    sed -n '/const LOGO: \[&str; 12\]/,/^    \];/p' "$source_file" \
        | grep -E '^        "' | sed 's/^    //' > "${work}/committed"
    if diff -u "${work}/committed" "${work}/expected" > "${work}/delta"; then
        echo "logo grid matches assets/brand/logo-mark.svg"
    else
        echo "error: the committed logo grid no longer matches the SVG" >&2
        cat "${work}/delta" >&2
        exit 1
    fi
else
    generate
fi

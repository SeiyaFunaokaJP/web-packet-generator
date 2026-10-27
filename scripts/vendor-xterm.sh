#!/usr/bin/env bash
# Refreshes the vendored xterm.js (crates/wpg-web/vendor/xterm) from npm.
#
# The web terminal page (ADR-0011) loads these files from the wpg binary itself:
# labs are often offline and the CSP only allows same-origin scripts, so no CDN.
#
#   scripts/vendor-xterm.sh            # versions below
#   XTERM=6.0.0 scripts/vendor-xterm.sh
#
# Needs Node.js / npm. Review the diff, then rebuild the UI (trunk build).
set -euo pipefail

XTERM="${XTERM:-6.0.0}"
FIT="${FIT:-0.11.0}"
WEB_LINKS="${WEB_LINKS:-0.12.0}"
SEARCH="${SEARCH:-0.16.0}"

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/crates/wpg-web/vendor/xterm"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cd "$work"
npm init -y >/dev/null
npm install --ignore-scripts --no-audit --no-fund \
  "@xterm/xterm@$XTERM" "@xterm/addon-fit@$FIT" \
  "@xterm/addon-web-links@$WEB_LINKS" "@xterm/addon-search@$SEARCH" >/dev/null
m="$work/node_modules/@xterm"

mkdir -p "$dest"
# Drop the source map references: the maps are not shipped.
for f in xterm/lib/xterm.js addon-fit/lib/addon-fit.js \
  addon-web-links/lib/addon-web-links.js addon-search/lib/addon-search.js; do
  sed '/^\/\/# sourceMappingURL=/d' "$m/$f" >"$dest/$(basename "$f")"
done
cp "$m/xterm/css/xterm.css" "$dest/xterm.css"
{
  cat "$m/xterm/LICENSE"
  printf '\n----\n\nThe add-ons (addon-fit, addon-web-links, addon-search) are under the same\nMIT license:\n\n'
  for a in addon-fit addon-web-links addon-search; do
    head -n 1 "$m/$a/LICENSE" | sed "s/^/$a: /"
  done
} >"$dest/LICENSE"
cat >"$dest/VERSIONS" <<EOF
@xterm/xterm $XTERM
@xterm/addon-fit $FIT
@xterm/addon-web-links $WEB_LINKS
@xterm/addon-search $SEARCH
EOF
echo "vendored into $dest:"
cat "$dest/VERSIONS"

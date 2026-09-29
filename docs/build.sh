#!/usr/bin/env bash
# Build both language books into docs/book/{en,ja} and a language-redirect index.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

if ! command -v mdbook >/dev/null 2>&1; then
  echo "mdbook not found. Install with: cargo install mdbook --locked" >&2
  exit 1
fi

for lang in en ja; do
  mdbook build "$lang"
done

cat > book/index.html <<'HTML'
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>wpg Documentation</title>
<script>
  // Redirect by browser language: ja -> ja/, otherwise en/.
  var lang = (navigator.language || "en").toLowerCase();
  location.replace(lang.indexOf("ja") === 0 ? "ja/" : "en/");
</script>
<noscript><meta http-equiv="refresh" content="0; url=en/"></noscript>
</head>
<body>
<h1>wpg Documentation</h1>
<ul>
  <li><a href="en/">English</a></li>
  <li><a href="ja/">日本語</a></li>
</ul>
</body>
</html>
HTML
echo "Built docs/book/en, docs/book/ja and docs/book/index.html"

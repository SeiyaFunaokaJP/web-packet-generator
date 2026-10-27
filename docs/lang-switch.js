// Language switcher of the documentation site (both mdBooks load it through
// `additional-js`). The English and Japanese books have the same page tree, so
// switching keeps the current page: /<base>/en/<page> <-> /<base>/ja/<page>.
// The choice is remembered for the landing page (docs/build.sh).
(function () {
  "use strict";
  var LANGS = [
    ["en", "English"],
    ["ja", "日本語"],
  ];
  var KEY = "wpg-docs-lang";
  var m = location.pathname.match(/^(.*\/)(en|ja)(\/.*)?$/);
  var bar = document.querySelector(".right-buttons");
  if (!m || !bar) {
    return;
  }
  var base = m[1];
  var current = m[2];
  var page = m[3] || "/";

  var nav = document.createElement("nav");
  nav.className = "wpg-lang";
  nav.setAttribute("aria-label", current === "ja" ? "言語" : "Language");
  LANGS.forEach(function (l) {
    var a = document.createElement("a");
    a.textContent = l[1];
    a.lang = l[0];
    a.hreflang = l[0];
    a.href = base + l[0] + page + location.hash;
    if (l[0] === current) {
      a.setAttribute("aria-current", "true");
    }
    a.addEventListener("click", function () {
      try {
        localStorage.setItem(KEY, l[0]);
      } catch (e) {
        // storage blocked: the link still works
      }
    });
    nav.appendChild(a);
  });
  bar.insertBefore(nav, bar.firstChild);

  var style = document.createElement("style");
  style.textContent =
    ".wpg-lang{display:inline-flex;gap:.6em;align-items:center;margin-right:.8em;font-size:1.4rem}" +
    ".wpg-lang a{color:var(--icons);text-decoration:none}" +
    ".wpg-lang a:hover{color:var(--icons-hover)}" +
    ".wpg-lang a[aria-current]{color:var(--fg);font-weight:600}" +
    "@media (max-width:600px){.wpg-lang{font-size:1.2rem;gap:.4em}}";
  document.head.appendChild(style);
})();

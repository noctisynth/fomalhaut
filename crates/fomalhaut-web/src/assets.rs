//! Embedded assets used to validate the native WebKitGTK host.

/// Content Security Policy applied to every prototype resource response.
pub const PROTOTYPE_CSP: &str = "default-src 'none'; script-src fomalhaut:; style-src fomalhaut:; img-src fomalhaut:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

/// Fixed security headers returned by the prototype resource scheme.
pub const PROTOTYPE_HEADERS: [(&str, &str); 3] = [
    ("Content-Security-Policy", PROTOTYPE_CSP),
    ("Cross-Origin-Opener-Policy", "same-origin"),
    ("Cache-Control", "no-store"),
];

const INDEX_HTML: &[u8] = br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Fomalhaut WebKitGTK prototype</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <main>
    <p class="eyebrow">FOMALHAUT / WEBKITGTK</p>
    <h1>Native host prototype</h1>
    <p>This page validates the isolated resource scheme and protocol bridge. It is not a login screen.</p>
    <pre id="bridge-status" aria-live="polite">Connecting to the Rust host...</pre>
  </main>
  <script src="app.js"></script>
</body>
</html>
"#;

const STYLE_CSS: &[u8] = br#":root {
  color-scheme: dark;
  font-family: system-ui, sans-serif;
  background: #07131d;
  color: #e6f5ff;
}

body {
  min-height: 100vh;
  margin: 0;
  display: grid;
  place-items: center;
  background: radial-gradient(circle at top, #183a4d, #07131d 62%);
}

main {
  width: min(42rem, calc(100vw - 4rem));
}

.eyebrow {
  color: #75d5ff;
  letter-spacing: .18em;
}

h1 {
  font-size: clamp(2.5rem, 7vw, 5.5rem);
  margin: .2em 0;
}

pre {
  margin-top: 2rem;
  padding: 1rem;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  background: rgb(0 0 0 / 35%);
  border: 1px solid rgb(117 213 255 / 35%);
  border-radius: .75rem;
}
"#;

const APP_JS: &[u8] = br#"'use strict';

const status = document.getElementById('bridge-status');

window.addEventListener('fomalhaut:event', (event) => {
  status.textContent = JSON.stringify(event.detail, null, 2);
});

async function probeHost() {
  const request = {
    protocol: 1,
    id: 1,
    method: 'state.get',
    params: {},
  };

  try {
    const response = await window.webkit.messageHandlers.fomalhaut.postMessage(request);
    status.textContent = JSON.stringify(response, null, 2);
  } catch (_error) {
    status.textContent = 'The native protocol bridge rejected the probe.';
  }
}

void probeHost();
"#;

/// One immutable resource exposed by the prototype scheme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinAsset {
    body: &'static [u8],
    content_type: &'static str,
}

impl BuiltinAsset {
    /// Returns the immutable resource bytes.
    #[must_use]
    pub const fn body(self) -> &'static [u8] {
        self.body
    }

    /// Returns the fixed MIME type, including UTF-8 where applicable.
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        self.content_type
    }
}

/// Resolves an exact prototype URI without decoding or path normalization.
#[must_use]
pub const fn resolve_builtin_asset(uri: &str) -> Option<BuiltinAsset> {
    let (body, content_type) = match uri.as_bytes() {
        b"fomalhaut://theme/" | b"fomalhaut://theme/index.html" => {
            (INDEX_HTML, "text/html; charset=utf-8")
        }
        b"fomalhaut://theme/style.css" => (STYLE_CSS, "text/css; charset=utf-8"),
        b"fomalhaut://theme/app.js" => (APP_JS, "application/javascript"),
        _ => return None,
    };
    Some(BuiltinAsset { body, content_type })
}

#[cfg(test)]
mod tests {
    use super::{PROTOTYPE_CSP, PROTOTYPE_HEADERS, resolve_builtin_asset};

    #[test]
    fn resolves_only_exact_allowlisted_uris() {
        let index = resolve_builtin_asset("fomalhaut://theme/")
            .expect("the prototype index URI is allowlisted");
        assert_eq!(index.content_type(), "text/html; charset=utf-8");
        assert!(!index.body().is_empty());

        assert!(resolve_builtin_asset("fomalhaut://theme/app.js").is_some());
        assert!(resolve_builtin_asset("fomalhaut://theme/style.css").is_some());
        assert!(resolve_builtin_asset("fomalhaut://theme/../secret").is_none());
        assert!(resolve_builtin_asset("fomalhaut://theme/%2e%2e/secret").is_none());
        assert!(resolve_builtin_asset("fomalhaut://other/index.html").is_none());
        assert!(resolve_builtin_asset("https://example.com/").is_none());
    }

    #[test]
    fn prototype_csp_has_no_network_or_inline_script_escape() {
        assert!(PROTOTYPE_CSP.contains("default-src 'none'"));
        assert!(PROTOTYPE_CSP.contains("connect-src 'none'"));
        assert!(PROTOTYPE_CSP.contains("script-src fomalhaut:"));
        assert!(PROTOTYPE_CSP.contains("style-src fomalhaut:"));
        assert!(!PROTOTYPE_CSP.contains("unsafe-inline"));
        assert!(!PROTOTYPE_CSP.contains("http:"));
        assert!(!PROTOTYPE_CSP.contains("https:"));
    }

    #[test]
    fn prototype_headers_avoid_webkit_custom_scheme_nosniff_incompatibility() {
        assert!(
            PROTOTYPE_HEADERS
                .iter()
                .all(|(name, _)| !name.eq_ignore_ascii_case("X-Content-Type-Options"))
        );
    }
}

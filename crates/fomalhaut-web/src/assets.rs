//! Embedded minimal theme used by the native WebKitGTK host.

/// Content Security Policy applied to every embedded theme resource response.
pub const EMBEDDED_THEME_CSP: &str = "default-src 'none'; script-src fomalhaut:; style-src fomalhaut:; img-src fomalhaut:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

/// Fixed security headers returned by the embedded theme resource scheme.
pub const EMBEDDED_THEME_HEADERS: [(&str, &str); 3] = [
    ("Content-Security-Policy", EMBEDDED_THEME_CSP),
    ("Cross-Origin-Opener-Policy", "same-origin"),
    ("Cache-Control", "no-store"),
];

const INDEX_HTML: &[u8] = br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Fomalhaut Login</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <main aria-labelledby="title">
    <p class="eyebrow">FOMALHAUT</p>
    <h1 id="title">Sign in</h1>
    <p class="introduction">Minimal example theme for the Fomalhaut frontend protocol.</p>

    <form id="login-form">
      <label id="credential-label" for="credential">Username</label>
      <input id="credential" name="username" type="text" autocomplete="username"
             autocapitalize="none" spellcheck="false" required autofocus>

      <label for="session">Session</label>
      <select id="session" name="session"></select>

      <div class="actions">
        <button id="submit" type="submit">Continue</button>
        <button id="cancel" type="button" hidden>Cancel</button>
      </div>
    </form>

    <p id="status" role="status" aria-live="polite">Connecting to the login service...</p>
    <ul id="messages" aria-live="polite"></ul>
    <p class="notice">This is the built-in example theme, not a fixed product interface.</p>
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
  width: min(28rem, calc(100vw - 3rem));
  padding: clamp(1.5rem, 5vw, 3rem);
  background: rgb(4 13 20 / 78%);
  border: 1px solid rgb(117 213 255 / 28%);
  border-radius: 1rem;
  box-shadow: 0 1.5rem 5rem rgb(0 0 0 / 35%);
}

.eyebrow {
  color: #75d5ff;
  letter-spacing: .18em;
}

h1 {
  font-size: clamp(2.25rem, 7vw, 4rem);
  margin: .15em 0;
}

.introduction,
.notice {
  color: #a8c4d3;
}

form {
  display: grid;
  gap: .65rem;
  margin-top: 2rem;
}

label {
  margin-top: .45rem;
  font-weight: 650;
}

input,
select,
button {
  box-sizing: border-box;
  width: 100%;
  min-height: 3rem;
  border: 1px solid rgb(117 213 255 / 42%);
  border-radius: .55rem;
  font: inherit;
}

input,
select {
  padding: .65rem .8rem;
  color: #e6f5ff;
  background: #0b202c;
}

button {
  padding: .65rem 1rem;
  color: #06131c;
  background: #75d5ff;
  font-weight: 750;
  cursor: pointer;
}

button#cancel {
  color: #e6f5ff;
  background: transparent;
}

button:disabled,
input:disabled,
select:disabled {
  cursor: wait;
  opacity: .55;
}

.actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: .75rem;
  margin-top: .8rem;
}

[hidden] {
  display: none !important;
}

#status {
  min-height: 1.5em;
  margin-top: 1.5rem;
}

#messages {
  max-height: 8rem;
  padding-left: 1.25rem;
  overflow: auto;
  color: #ffd2a8;
}

.notice {
  margin: 2rem 0 0;
  font-size: .85rem;
}
"#;

const APP_JS: &[u8] = br#"'use strict';

const form = document.getElementById('login-form');
const credentialLabel = document.getElementById('credential-label');
const credential = document.getElementById('credential');
const session = document.getElementById('session');
const submit = document.getElementById('submit');
const cancel = document.getElementById('cancel');
const status = document.getElementById('status');
const messages = document.getElementById('messages');

let nextRequestId = 1;
let activePrompt = null;
let busy = false;
let terminal = false;
let lastSequence = 0;

function setStatus(message) {
  status.textContent = message;
}

function updateControls() {
  const disabled = busy || terminal;
  credential.disabled = disabled;
  session.disabled = disabled;
  submit.disabled = disabled;
  cancel.disabled = disabled;
}

function setBusy(value) {
  busy = value;
  updateControls();
}

function showUsernameInput() {
  activePrompt = null;
  credential.value = '';
  credential.type = 'text';
  credential.name = 'username';
  credential.autocomplete = 'username';
  credentialLabel.textContent = 'Username';
  submit.textContent = 'Continue';
  cancel.hidden = true;
  if (!terminal) {
    credential.focus();
  }
}

function showPrompt(prompt) {
  activePrompt = prompt;
  credential.value = '';
  credential.type = prompt.kind === 'secret' ? 'password' : 'text';
  credential.name = 'response';
  credential.autocomplete = 'off';
  credentialLabel.textContent = prompt.message;
  submit.textContent = 'Respond';
  cancel.hidden = false;
  if (!terminal) {
    credential.focus();
  }
}

function addMessage(message) {
  const item = document.createElement('li');
  item.textContent = message.text;
  item.dataset.level = message.level;
  messages.append(item);
  while (messages.children.length > 16) {
    messages.firstElementChild.remove();
  }
}

function setSessions(snapshot) {
  session.replaceChildren();
  for (const item of snapshot.sessions) {
    const option = document.createElement('option');
    option.value = item.id;
    option.textContent = `${item.name} (${item.kind})`;
    option.selected = item.id === snapshot.selectedSessionId;
    session.append(option);
  }
}

function applySnapshot(snapshot) {
  setSessions(snapshot);
  messages.replaceChildren();
  for (const message of snapshot.messages) {
    addMessage(message);
  }
  if (snapshot.prompt) {
    showPrompt(snapshot.prompt);
  } else if (snapshot.authentication === 'starting_session'
      || snapshot.authentication === 'started') {
    terminal = true;
    setStatus('Starting the selected session...');
  } else {
    showUsernameInput();
    setStatus('Enter your username to continue.');
  }
  updateControls();
}

async function sendRequest(method, params) {
  if (nextRequestId > Number.MAX_SAFE_INTEGER) {
    terminal = true;
    updateControls();
    setStatus('The frontend request counter is exhausted.');
    return null;
  }
  const request = { protocol: 1, id: nextRequestId, method, params };
  nextRequestId += 1;
  setBusy(true);
  try {
    const response = await window.webkit.messageHandlers.fomalhaut.postMessage(request);
    if (!response.ok) {
      setStatus(response.error.message);
    }
    return response;
  } catch (_error) {
    terminal = true;
    setStatus('The login service bridge is unavailable.');
    return null;
  } finally {
    setBusy(false);
  }
}

async function refreshState() {
  const response = await sendRequest('state.get', {});
  if (response && response.ok) {
    applySnapshot(response.result);
  }
}

form.addEventListener('submit', async (event) => {
  event.preventDefault();
  if (busy || terminal) {
    return;
  }

  let value = credential.value;
  credential.value = '';
  if (activePrompt) {
    const promptId = activePrompt.promptId;
    activePrompt = null;
    const pending = sendRequest('auth.respond', { promptId, response: value });
    value = '';
    const response = await pending;
    if (response && !response.ok) {
      await refreshState();
    }
  } else {
    const pending = sendRequest('auth.begin', { username: value });
    value = '';
    const response = await pending;
    if (response && !response.ok) {
      showUsernameInput();
    }
  }
});

session.addEventListener('change', async () => {
  const response = await sendRequest('session.select', { sessionId: session.value });
  if (response && !response.ok) {
    await refreshState();
  }
});

cancel.addEventListener('click', async () => {
  credential.value = '';
  activePrompt = null;
  const response = await sendRequest('auth.cancel', {});
  if (response && !response.ok) {
    await refreshState();
  }
});

window.addEventListener('fomalhaut:event', (event) => {
  const message = event.detail;
  if (message.protocol !== 1 || message.sequence <= lastSequence) {
    return;
  }
  lastSequence = message.sequence;

  switch (message.event) {
    case 'auth.prompt':
      showPrompt(message.data);
      setStatus('Authentication requires a response.');
      break;
    case 'auth.message':
      addMessage(message.data);
      break;
    case 'auth.succeeded':
      terminal = true;
      updateControls();
      setStatus('Authentication succeeded. Starting the selected session...');
      break;
    case 'auth.failed':
      showUsernameInput();
      setStatus('Authentication failed. Try again.');
      break;
    case 'auth.cancelled':
      showUsernameInput();
      setStatus('Authentication was cancelled.');
      break;
    case 'session.selected':
      session.value = message.data.sessionId;
      setStatus('Session selection updated.');
      break;
    case 'session.started':
      terminal = true;
      updateControls();
      setStatus('Session started.');
      break;
    case 'state.changed':
      if (message.data.state === 'disconnected') {
        terminal = true;
        updateControls();
        setStatus('The login service disconnected.');
      }
      break;
    default:
      break;
  }
});

void refreshState();
"#;

/// One immutable resource exposed by the embedded theme scheme.
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

/// Resolves an exact embedded-theme URI without decoding or path normalization.
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
    use super::{EMBEDDED_THEME_CSP, EMBEDDED_THEME_HEADERS, resolve_builtin_asset};

    #[test]
    fn resolves_only_exact_allowlisted_uris() {
        let index = resolve_builtin_asset("fomalhaut://theme/")
            .expect("the embedded theme index URI is allowlisted");
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
    fn embedded_theme_csp_has_no_network_or_inline_script_escape() {
        assert!(EMBEDDED_THEME_CSP.contains("default-src 'none'"));
        assert!(EMBEDDED_THEME_CSP.contains("connect-src 'none'"));
        assert!(EMBEDDED_THEME_CSP.contains("script-src fomalhaut:"));
        assert!(EMBEDDED_THEME_CSP.contains("style-src fomalhaut:"));
        assert!(!EMBEDDED_THEME_CSP.contains("unsafe-inline"));
        assert!(!EMBEDDED_THEME_CSP.contains("http:"));
        assert!(!EMBEDDED_THEME_CSP.contains("https:"));
    }

    #[test]
    fn embedded_theme_headers_avoid_webkit_custom_scheme_nosniff_incompatibility() {
        assert!(
            EMBEDDED_THEME_HEADERS
                .iter()
                .all(|(name, _)| !name.eq_ignore_ascii_case("X-Content-Type-Options"))
        );
    }

    #[test]
    fn minimal_theme_exposes_only_protocol_driven_login_controls() {
        let index = resolve_builtin_asset("fomalhaut://theme/index.html")
            .expect("the embedded login page is allowlisted");
        let index = std::str::from_utf8(index.body()).expect("embedded HTML is valid UTF-8");
        let script = resolve_builtin_asset("fomalhaut://theme/app.js")
            .expect("the embedded login script is allowlisted");
        let script =
            std::str::from_utf8(script.body()).expect("embedded JavaScript is valid UTF-8");

        for control in [
            "id=\"login-form\"",
            "for=\"credential\"",
            "id=\"session\"",
            "aria-live=\"polite\"",
        ] {
            assert!(index.contains(control));
        }
        for method in [
            "'state.get'",
            "'session.select'",
            "'auth.begin'",
            "'auth.respond'",
            "'auth.cancel'",
        ] {
            assert!(script.contains(method));
        }
        assert!(script.contains("prompt.kind === 'secret'"));
        assert!(script.contains("message.sequence <= lastSequence"));
        assert!(!script.contains("innerHTML"));
        assert!(!script.contains("fetch("));
        assert!(!script.contains("http://"));
        assert!(!script.contains("https://"));

        let clear = script
            .find("credential.value = '';")
            .expect("the credential DOM value is cleared before a request");
        let respond = script
            .find("sendRequest('auth.respond'")
            .expect("the prompt response is sent through the typed bridge");
        let release = script[respond..]
            .find("value = '';")
            .map(|offset| respond + offset)
            .expect("the local credential reference is released");
        let wait = script[release..]
            .find("await pending")
            .map(|offset| release + offset)
            .expect("the bridge request is awaited after local release");
        assert!(clear < respond && respond < release && release < wait);
    }
}

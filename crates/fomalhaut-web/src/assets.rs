//! Embedded minimal theme used by the native WebKitGTK host.

/// Content Security Policy applied to every embedded theme resource response.
pub const THEME_CSP: &str = "default-src 'none'; script-src fomalhaut:; style-src fomalhaut:; img-src fomalhaut:; font-src fomalhaut:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

/// Fixed security headers returned by the embedded theme resource scheme.
pub const THEME_HEADERS: [(&str, &str); 3] = [
    ("Content-Security-Policy", THEME_CSP),
    ("Cross-Origin-Opener-Policy", "same-origin"),
    ("Cache-Control", "no-store"),
];

const INDEX_HTML: &[u8] = br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Fomalhaut</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <main aria-labelledby="title">
    <p class="eyebrow">FOMALHAUT</p>
    <h1 id="title">Sign in</h1>
    <p id="introduction" class="introduction">Minimal example theme for the Fomalhaut frontend protocol.</p>

    <section id="locker-identity" aria-labelledby="locker-identity-name" hidden>
      <div id="locker-avatar" class="user-fallback" aria-hidden="true">?</div>
      <h2 id="locker-identity-name"></h2>
      <p id="locker-username" class="introduction"></p>
    </section>

    <section id="known-users" aria-labelledby="known-users-title" hidden>
      <h2 id="known-users-title">Users</h2>
      <div id="user-list"></div>
      <button id="other-user" type="button">Other user</button>
    </section>

    <form id="login-form">
      <label id="credential-label" for="credential">Username</label>
      <input id="credential" name="username" type="text" autocomplete="username"
             autocapitalize="none" spellcheck="false" required autofocus>

      <div id="session-control">
        <label id="session-label" for="session">Session</label>
        <select id="session" name="session"></select>
      </div>

      <div class="actions">
        <button id="submit" type="submit">Continue</button>
        <button id="cancel" type="button" hidden>Cancel</button>
      </div>
    </form>

    <p id="status" role="status" aria-live="polite">Connecting to the login service...</p>
    <ul id="messages" aria-live="polite"></ul>
    <p id="notice" class="notice">This is the built-in example theme, not a fixed product interface.</p>
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

h2 {
  margin: 1.5rem 0 .65rem;
  font-size: 1rem;
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

#session-control {
  display: grid;
  gap: .65rem;
}

#locker-identity {
  justify-items: center;
  margin-top: 1.5rem;
  text-align: center;
}

#locker-identity:not([hidden]) {
  display: grid;
}

#locker-identity .user-fallback {
  width: 4.5rem;
  height: 4.5rem;
  font-size: 1.5rem;
}

#locker-identity h2,
#locker-identity p {
  margin: .65rem 0 0;
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

#user-list {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr));
  gap: .65rem;
}

.user-choice {
  display: grid;
  justify-items: center;
  gap: .4rem;
  color: #e6f5ff;
  background: #0b202c;
}

.user-choice img,
.user-fallback {
  width: 3rem;
  height: 3rem;
  border-radius: 50%;
  object-fit: cover;
  background: #183a4d;
}

.user-fallback {
  display: grid;
  place-items: center;
}

#other-user {
  margin-top: .65rem;
  color: #e6f5ff;
  background: transparent;
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

const APP_JS: &[u8] = r#"'use strict';

const form = document.getElementById('login-form');
const title = document.getElementById('title');
const introduction = document.getElementById('introduction');
const credentialLabel = document.getElementById('credential-label');
const credential = document.getElementById('credential');
const lockerIdentity = document.getElementById('locker-identity');
const lockerAvatar = document.getElementById('locker-avatar');
const lockerIdentityName = document.getElementById('locker-identity-name');
const lockerUsername = document.getElementById('locker-username');
const knownUsers = document.getElementById('known-users');
const knownUsersTitle = document.getElementById('known-users-title');
const userList = document.getElementById('user-list');
const otherUser = document.getElementById('other-user');
const sessionControl = document.getElementById('session-control');
const sessionLabel = document.getElementById('session-label');
const session = document.getElementById('session');
const submit = document.getElementById('submit');
const cancel = document.getElementById('cancel');
const status = document.getElementById('status');
const messages = document.getElementById('messages');
const notice = document.getElementById('notice');

const translations = {
  en: {
    signIn: 'Sign in',
    introduction: 'Minimal example theme for the Fomalhaut frontend protocol.',
    users: 'Users',
    otherUser: 'Other user',
    username: 'Username',
    password: 'Password',
    session: 'Session',
    continue: 'Continue',
    cancel: 'Cancel',
    connecting: 'Connecting to the login service...',
    notice: 'This is the built-in example theme, not a fixed product interface.',
    credential: 'Credential',
    tryAgain: 'Try again',
    waiting: 'Waiting...',
    respond: 'Respond',
    startingSession: 'Starting the selected session...',
    enterUsername: 'Enter your username to continue.',
    sessionLocked: 'Session locked',
    lockFailed: 'The native session lock failed.',
    sessionUnlocked: 'Session unlocked.',
    authSucceededUnlocking: 'Authentication succeeded. Unlocking...',
    authRequiresResponse: 'Authentication requires a response.',
    securingSession: 'Securing this session...',
    authFailed: 'Authentication failed. Try again.',
    authWaiting: 'Waiting for the authentication service...',
    requestCounterExhausted: 'The frontend request counter is exhausted.',
    bridgeUnavailable: 'The Fomalhaut host bridge is unavailable.',
    authSucceededStarting: 'Authentication succeeded. Starting the selected session...',
    authCancelled: 'Authentication was cancelled.',
    sessionSelectionUpdated: 'Session selection updated.',
    sessionStarted: 'Session started.',
    lockedWaiting: 'Session locked. Waiting for authentication...',
    authServiceFailure: 'The authentication service could not complete the request.'
  },
  'zh-CN': {
    signIn: '登录',
    introduction: 'Fomalhaut 前端协议的内置最小示例主题。',
    users: '用户',
    otherUser: '其他用户',
    username: '用户名',
    password: '密码',
    session: '会话',
    continue: '继续',
    cancel: '取消',
    connecting: '正在连接登录服务…',
    notice: '这是内置示例主题，不是固定的产品界面。',
    credential: '认证凭据',
    tryAgain: '重试',
    waiting: '请稍候…',
    respond: '提交',
    startingSession: '正在启动所选会话…',
    enterUsername: '输入用户名以继续。',
    sessionLocked: '会话已锁定',
    lockFailed: '原生会话锁失败。',
    sessionUnlocked: '会话已解锁。',
    authSucceededUnlocking: '认证成功，正在解锁…',
    authRequiresResponse: '认证需要输入响应。',
    securingSession: '正在保护当前会话…',
    authFailed: '认证失败，请重试。',
    authWaiting: '正在等待认证服务…',
    requestCounterExhausted: '前端请求计数器已耗尽。',
    bridgeUnavailable: 'Fomalhaut 宿主桥接不可用。',
    authSucceededStarting: '认证成功，正在启动所选会话…',
    authCancelled: '认证已取消。',
    sessionSelectionUpdated: '会话选择已更新。',
    sessionStarted: '会话已启动。',
    lockedWaiting: '会话已锁定，正在等待认证…',
    authServiceFailure: '认证服务无法完成请求。'
  }
};

let nextRequestId = 1;
let activePrompt = null;
let mode = null;
let busy = false;
let terminal = false;
let retryAvailable = false;
let lastSequence = 0;
let locale = 'en';

function browserLocale() {
  const languages = Array.isArray(navigator.languages)
    ? navigator.languages
    : [navigator.language];
  return languages.some((language) => {
    if (typeof language !== 'string') {
      return false;
    }
    const normalized = language.trim().replaceAll('_', '-').toLowerCase();
    return normalized === 'zh' || normalized.startsWith('zh-');
  }) ? 'zh-CN' : 'en';
}

function text(key) {
  return translations[locale][key];
}

const passwordPromptPattern = /^password\s*:?\s*$/i;
const passwordForPromptPattern = /^password\s+for\s+[^:\s](?:[^:\r\n]*[^:\s])?\s*:?\s*$/i;

function promptLabel(prompt) {
  if (prompt.kind === 'secret' &&
      (passwordPromptPattern.test(prompt.message) ||
       passwordForPromptPattern.test(prompt.message))) {
    return text('password');
  }
  return prompt.message;
}

function applyLocale(nextLocale) {
  locale = nextLocale === 'zh-CN' ? 'zh-CN' : 'en';
  document.documentElement.lang = locale;
  title.textContent = text('signIn');
  introduction.textContent = text('introduction');
  knownUsersTitle.textContent = text('users');
  otherUser.textContent = text('otherUser');
  credentialLabel.textContent = text('username');
  sessionLabel.textContent = text('session');
  submit.textContent = text('continue');
  cancel.textContent = text('cancel');
  status.textContent = text('connecting');
  notice.textContent = text('notice');
}

function setStatus(message) {
  status.textContent = message;
}

function updateControls() {
  const disabled = busy || terminal;
  credential.disabled = disabled || (mode === 'locker' && activePrompt === null);
  session.disabled = disabled || mode !== 'greeter';
  submit.disabled = disabled || (mode === 'locker' && activePrompt === null && !retryAvailable);
  cancel.disabled = disabled;
  otherUser.disabled = disabled || mode !== 'greeter' || activePrompt !== null;
  for (const button of userList.querySelectorAll('button')) {
    button.disabled = disabled || activePrompt !== null;
  }
}

function setBusy(value) {
  busy = value;
  updateControls();
}

function showUsernameInput() {
  if (mode !== 'greeter') {
    return;
  }
  activePrompt = null;
  retryAvailable = false;
  credential.value = '';
  credential.type = 'text';
  credential.name = 'username';
  credential.autocomplete = 'username';
  credentialLabel.textContent = text('username');
  submit.textContent = text('continue');
  cancel.hidden = true;
  if (!terminal) {
    credential.focus();
  }
  updateControls();
}

function showLockerWaiting(canRetry) {
  activePrompt = null;
  retryAvailable = canRetry;
  credential.value = '';
  credential.type = 'password';
  credential.name = 'response';
  credential.autocomplete = 'off';
  credentialLabel.textContent = text('credential');
  submit.textContent = canRetry ? text('tryAgain') : text('waiting');
  cancel.hidden = true;
  updateControls();
}

function showPrompt(prompt) {
  activePrompt = prompt;
  retryAvailable = false;
  credential.value = '';
  credential.type = prompt.kind === 'secret' ? 'password' : 'text';
  credential.name = 'response';
  credential.autocomplete = 'off';
  credentialLabel.textContent = promptLabel(prompt);
  submit.textContent = text('respond');
  cancel.hidden = false;
  if (!terminal) {
    credential.focus();
  }
  updateControls();
}

function setLockerIdentity(identity) {
  lockerIdentityName.textContent = identity.displayName;
  lockerUsername.textContent = identity.username;
  lockerAvatar.textContent = identity.displayName.slice(0, 1).toLocaleUpperCase() || '?';
  lockerIdentity.hidden = false;
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

function selectKnownUser(username) {
  if (busy || terminal || activePrompt) {
    return;
  }
  credential.value = username;
  credential.focus();
}

function setUsers(snapshot) {
  userList.replaceChildren();
  for (const user of snapshot.users) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'user-choice';
    button.addEventListener('click', () => selectKnownUser(user.username));

    const fallback = document.createElement('span');
    fallback.className = 'user-fallback';
    fallback.setAttribute('aria-hidden', 'true');
    fallback.textContent = user.displayName.slice(0, 1).toLocaleUpperCase();
    button.append(fallback);
    if (user.avatarUrl) {
      const avatar = document.createElement('img');
      avatar.alt = '';
      avatar.src = user.avatarUrl;
      avatar.addEventListener('load', () => fallback.remove(), { once: true });
      avatar.addEventListener('error', () => avatar.remove(), { once: true });
      button.prepend(avatar);
    }

    const label = document.createElement('span');
    label.textContent = user.displayName;
    button.append(label);
    userList.append(button);
  }
  knownUsers.hidden = snapshot.users.length === 0;
  updateControls();
}

function applySnapshot(snapshot) {
  applyLocale(snapshot.locale);
  mode = snapshot.mode;
  lastSequence = Math.max(lastSequence, snapshot.sequence);
  terminal = false;
  messages.replaceChildren();
  for (const message of snapshot.messages) {
    addMessage(message);
  }

  if (mode === 'greeter') {
    title.textContent = text('signIn');
    lockerIdentity.hidden = true;
    sessionControl.hidden = false;
    setUsers(snapshot);
    setSessions(snapshot);
    if (snapshot.prompt) {
      showPrompt(snapshot.prompt);
    } else if (snapshot.login === 'starting_session' || snapshot.login === 'started') {
      terminal = true;
      setStatus(text('startingSession'));
    } else {
      showUsernameInput();
      setStatus(text('enterUsername'));
    }
    updateControls();
    return false;
  }

  title.textContent = text('sessionLocked');
  knownUsers.hidden = true;
  sessionControl.hidden = true;
  setLockerIdentity(snapshot.identity);
  if (snapshot.lock === 'failed') {
    terminal = true;
    showLockerWaiting(false);
    setStatus(text('lockFailed'));
  } else if (snapshot.lock === 'released') {
    terminal = true;
    showLockerWaiting(false);
    setStatus(text('sessionUnlocked'));
  } else if (snapshot.lock === 'unlocking' || snapshot.authentication === 'authenticated') {
    terminal = true;
    showLockerWaiting(false);
    setStatus(text('authSucceededUnlocking'));
  } else if (snapshot.prompt) {
    showPrompt(snapshot.prompt);
    setStatus(text('authRequiresResponse'));
  } else if (snapshot.lock === 'acquiring') {
    showLockerWaiting(false);
    setStatus(text('securingSession'));
  } else if (snapshot.authentication === 'failed') {
    showLockerWaiting(true);
    setStatus(text('authFailed'));
  } else {
    showLockerWaiting(false);
    setStatus(text('authWaiting'));
  }
  updateControls();
  return snapshot.lock === 'locked' && snapshot.authentication === 'idle';
}

async function sendRequest(method, params) {
  if (nextRequestId > Number.MAX_SAFE_INTEGER) {
    terminal = true;
    updateControls();
    setStatus(text('requestCounterExhausted'));
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
    setStatus(text('bridgeUnavailable'));
    return null;
  } finally {
    setBusy(false);
  }
}

async function refreshState() {
  const response = await sendRequest('state.get', {});
  if (response && response.ok) {
    const beginLockerAuthentication = applySnapshot(response.result);
    if (beginLockerAuthentication) {
      await sendRequest('auth.begin', {});
    }
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
    const params = mode === 'locker' ? {} : { username: value };
    const pending = sendRequest('auth.begin', params);
    value = '';
    const response = await pending;
    if (response && !response.ok) {
      if (mode === 'locker') {
        showLockerWaiting(true);
      } else {
        showUsernameInput();
      }
    }
  }
});

session.addEventListener('change', async () => {
  if (mode !== 'greeter') {
    return;
  }
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

otherUser.addEventListener('click', () => {
  if (mode === 'greeter' && !busy && !terminal && !activePrompt) {
    credential.value = '';
    credential.focus();
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
      setStatus(text('authRequiresResponse'));
      break;
    case 'auth.message':
      addMessage(message.data);
      break;
    case 'auth.succeeded':
      terminal = true;
      updateControls();
      setStatus(mode === 'locker'
        ? text('authSucceededUnlocking')
        : text('authSucceededStarting'));
      break;
    case 'auth.failed':
      if (mode === 'locker') {
        showLockerWaiting(true);
      } else {
        showUsernameInput();
      }
      setStatus(text('authFailed'));
      break;
    case 'auth.cancelled':
      if (mode === 'locker') {
        showLockerWaiting(true);
      } else {
        showUsernameInput();
      }
      setStatus(text('authCancelled'));
      break;
    case 'session.selected':
      if (mode === 'greeter') {
        session.value = message.data.sessionId;
        setStatus(text('sessionSelectionUpdated'));
      }
      break;
    case 'session.started':
      terminal = true;
      updateControls();
      setStatus(text('sessionStarted'));
      break;
    case 'lock.acquired':
      if (mode === 'locker') {
        showLockerWaiting(false);
        setStatus(text('lockedWaiting'));
        void sendRequest('auth.begin', {});
      }
      break;
    case 'lock.failed':
      if (mode === 'locker') {
        terminal = true;
        showLockerWaiting(false);
        setStatus(text('lockFailed'));
      }
      break;
    case 'lock.released':
      if (mode === 'locker') {
        terminal = true;
        showLockerWaiting(false);
        setStatus(text('sessionUnlocked'));
      }
      break;
    case 'state.changed':
      if (message.data.state === 'failed') {
        if (mode === 'locker') {
          showLockerWaiting(true);
        } else {
          showUsernameInput();
        }
        setStatus(text('authServiceFailure'));
      }
      break;
    default:
      break;
  }
});

applyLocale(browserLocale());
void refreshState();
"#
.as_bytes();

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
    use super::{THEME_CSP, THEME_HEADERS, resolve_builtin_asset};

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
        assert!(THEME_CSP.contains("default-src 'none'"));
        assert!(THEME_CSP.contains("connect-src 'none'"));
        assert!(THEME_CSP.contains("script-src fomalhaut:"));
        assert!(THEME_CSP.contains("style-src fomalhaut:"));
        assert!(THEME_CSP.contains("font-src fomalhaut:"));
        assert!(!THEME_CSP.contains("unsafe-inline"));
        assert!(!THEME_CSP.contains("http:"));
        assert!(!THEME_CSP.contains("https:"));
    }

    #[test]
    fn embedded_theme_headers_avoid_webkit_custom_scheme_nosniff_incompatibility() {
        assert!(
            THEME_HEADERS
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
            "id=\"known-users\"",
            "id=\"locker-identity\"",
            "for=\"credential\"",
            "id=\"session-control\"",
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
        assert!(script.contains("snapshot.users"));
        assert!(script.contains("snapshot.identity"));
        assert!(script.contains("snapshot.sequence"));
        assert!(script.contains("applyLocale(snapshot.locale)"));
        assert!(script.contains("document.documentElement.lang = locale"));
        assert!(script.contains("navigator.languages"));
        assert!(script.contains("'zh-CN':"));
        assert!(script.contains("password: '密码'"));
        assert!(script.contains("function promptLabel(prompt)"));
        assert!(script.contains("credentialLabel.textContent = promptLabel(prompt)"));
        assert!(script.contains("认证失败，请重试。"));
        assert!(script.contains("user.avatarUrl"));
        assert!(script.contains("message.sequence <= lastSequence"));
        assert!(script.contains("mode === 'locker' ? {} : { username: value }"));
        assert!(script.contains("case 'lock.acquired':"));
        assert!(script.contains("case 'lock.released':"));
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

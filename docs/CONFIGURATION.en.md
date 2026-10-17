# Fomalhaut configuration and external themes

[简体中文](CONFIGURATION.md)

Fomalhaut always reads `/etc/fomalhaut/config.toml`. If the file does not exist,
it uses the embedded minimal theme and the default system session directories.
If the file exists but cannot be read, contains unknown fields, or fails
validation, Fomalhaut exits with a nonzero status.

## AUR installation, source uninstallation, and migration

Arch Linux users can install three independent AUR packages together:

```sh
paru -S --removemake greetd-fomalhaut fomalhaut-lock fomalhaut-theme-nocturne
```

The packages install runtime dependencies, two `/usr/bin` binaries, locker PAM
and systemd assets, the static Nocturne theme, and integration examples. They
preserve `/etc/fomalhaut/config.toml` and `/etc/greetd/config.toml`. For a new
installation, merge the greetd/Cage example from
`/usr/share/doc/greetd-fomalhaut/greetd-config.toml` into your configuration.
The example resolves `dbus-run-session`, `cage`, and `fomalhaut` through PATH
rather than fixing their paths to `/usr/bin`. Enable the AUR theme explicitly:

```toml
[themes]
default = "nocturne"
```

Fomalhaut is a greeter for greetd. After reviewing and writing
`/etc/greetd/config.toml`, disable the existing display manager before enabling
greetd. Avoid enabling two display managers at once:

```sh
sudo systemctl disable <current-display-manager>.service
sudo systemctl enable greetd.service
```

Then reboot, or start greetd manually when it is safe to end the current
graphical session. pacman repeats this reminder during installation and
upgrades; the package itself preserves configuration and service state. Without
a Fomalhaut configuration file, both roles use safe defaults and the embedded
minimal theme.

If you previously ran the source installer, install the required AUR application
packages and theme first, then run this command from an updated source checkout:

```sh
./uninstall.sh
```

The script also works as a normal source uninstaller without any AUR packages.
It removes old binaries, the locker user unit, integration examples, and
verifiably source-managed Nocturne symlinks and releases under the default
`/usr/local` prefix. By default, it preserves Fomalhaut/greetd configuration and
backups. When `fomalhaut-theme-nocturne` is installed, theme selectors that
exactly reference old source paths are atomically migrated to `nocturne`. When
`greetd-fomalhaut` is installed, the preserved `[default_session].command` is
atomically migrated from the old greeter path to `/usr/bin/fomalhaut`. Without
an AUR replacement, configuration is preserved with a warning about potentially
stale references. Before deleting either TOML file, an unverified legacy theme,
or a PAM policy without an AUR replacement, the script lists the affected files
and asks for explicit `[y/N]` confirmation. Noninteractive runs always preserve
these files. The AUR locker's `/etc/pam.d/fomalhaut-lock` and any `.pacnew` file
are preserved.

The uninstaller leaves greetd running and does not scan user home directories.
Afterwards, check whether your niri or swayidle configuration still references
`/usr/local/bin/fomalhaut-lock`, reload the systemd user manager, and restart
greetd manually only when it is safe to end the current greeter session. Use
`--prefix` for a nondefault source prefix; `--system-root` is for isolated
validation only.

## Installing from source

The installer in the repository root builds release binaries and the React
reference theme with locked dependencies, then installs them into system
locations:

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0
```

### Display scaling

Use `--display-scale 1.5` when both roles should use the same page zoom. When
the greeter runs in a separate Cage session and the locker runs inside an
existing compositor such as niri, normally use the paired
`--greeter-scale`/`--locker-scale` options. Cage does not inherit desktop output
scaling, while the locker's compositor already handles logical coordinates and
output scale. The shared option cannot be combined with the role options, and
the role options must be supplied together. All scale values must be between
`0.5` and `4.0`.

Omitting the option uses `1.0` for a new configuration and preserves existing
values during updates. Cursor size is controlled separately by `--cursor-size`.

When creating `/etc/fomalhaut/config.toml` for the first time, the source
installer explicitly enables poweroff, reboot, and suspend. Updates preserve
the existing power policy without adding or expanding allowed actions.

### Interface language

Language configuration is optional. At startup, each role checks `LC_ALL`,
`LC_MESSAGES`, and `LANG` in that order. Chinese variants such as `zh`, `zh_CN`,
and `zh-TW` use Simplified Chinese; all other languages, `C`, `POSIX`, and
unrecognized values fall back to English. Encoding suffixes such as `.UTF-8`
and locale modifiers are ignored.

To set the language for both roles during source installation, pass:

```sh
./install.sh --language zh-CN
```

The option accepts only `en` and `zh-CN`. When omitted, a new installation does
not create `[locale]`, and an update preserves the administrator's existing
settings. You can also override the language globally:

```toml
[locale]
language = "zh-CN"
```

Currently, only `en` and `zh-CN` are accepted; other values fail strict
configuration validation. Restart the greeter or locker process after changing
configuration. The host includes the resolved language in `state.get.locale`
and uses it to select localized session names from Desktop Entries. Once the
snapshot arrives, themes must use that value instead of their initial browser
language guess. Fomalhaut's own interface supports English and Simplified
Chinese. Standard secret prompts from PAM modules, such as `Password` or
`Password for <target>`, appear as `Password` or `密码` for the current UI locale.
OTP, PIN, visible, custom, and informational or error messages retain their
original PAM text. Updates preserve existing `[locale]` configuration.

### New installation

Run the installer with scaling appropriate for your display:

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0
```

The installer leaves service enablement to the administrator. From a text
console, run the following command; save your work first if you are still in
a graphical session:

```sh
sudo systemctl enable --now greetd.service
```

### Migrating from another display manager

First run the installer without `--restart`:

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0
```

Save your work and switch to a text console. Check `display-manager.service`,
disable and stop the existing display manager, then enable greetd. For example,
to migrate from SDDM:

```sh
systemctl status display-manager.service
sudo systemctl disable --now sddm.service
sudo systemctl enable --now greetd.service
```

Replace `sddm.service` with the actual service, such as `gdm.service` or
`lightdm.service`. Avoid enabling two display managers at once.

Run the script as a regular user. Builds run without root privileges; the
installer invokes `sudo` only when writing system locations. By default, it
installs:

- `/usr/local/bin/fomalhaut`
- `/usr/local/bin/fomalhaut-lock`
- `/usr/local/lib/systemd/user/fomalhaut-lock.service`
- `/usr/local/share/doc/fomalhaut-lock/niri.kdl`
- `/usr/local/share/doc/fomalhaut-lock/swayidle.conf`
- `/etc/pam.d/fomalhaut-lock`
- `/usr/local/share/fomalhaut/themes/nocturne`
- `/etc/fomalhaut/config.toml`
- `/etc/greetd/config.toml`

The Rust, Bun canary, and GTK/WebKitGTK development tools required for source
builds must already be available. Before writing files on a real system, the
installer also checks for `/usr/bin/cage`, `/usr/bin/dbus-run-session`, and the
specified greeter account.

On Arch Linux, the installer checks missing build and runtime packages with
`pacman -T`. It prefers `paru`, then `yay`, then `sudo pacman`. Package
installation keeps interactive confirmation and does not trigger an implicit
full system upgrade. Manage Rust and Bun yourself; the installer does not
install, upgrade, or check their release channels through the system package
manager. AccountsService is optional. `--system-root` test mode leaves the
host's package state untouched.

Run the same command again to update. Existing binaries and ordinary theme
directories receive timestamped backups; theme releases are retained. Both TOML
files are parsed and validated before updates, and the installer changes only
its own fields, keeping `*.bak.<timestamp>.<process-id>` files in the same
directory. Invalid TOML, duplicate target fields, or symlink configuration files
cause the installer to refuse changes. Configuration preflight runs before
switching binaries and themes to avoid partial installation for detectable
configuration errors.

An existing `/etc/pam.d/fomalhaut-lock` that differs from the bundled policy is
preserved with a warning. If the built binaries, systemd unit, complete theme
tree, and generated configuration match the installation, rerunning creates no
new backups, theme releases, or replacement files. Backups and atomic switches
occur only when content changes.

In an interactive terminal, installer output uses colors for progress, success,
unchanged files, backups, and errors. Set `NO_COLOR=1` to disable colors;
redirected or piped output automatically uses plain text.

The installer does not inspect greetd enablement or detect and disable other
display managers. `--restart` runs `systemctl restart greetd` for updates to an
existing greetd installation. When you can safely end the current session and
greetd is already enabled, run:

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0 --restart
```

Use an absolute `--prefix` for a nondefault installation prefix.
`--system-root /tmp/fomalhaut-root` validates installation in an isolated
directory without invoking `sudo`; it cannot be combined with `--restart`.
See `./install.sh --help` for all options.

## Automatically unlocking the login keyring at login

After switching from SDDM, GDM, or another display manager to greetd, applications
that need stored credentials may display “Unlock Login Keyring” or “The login
keyring did not get unlocked when you logged into your computer.” Check greetd's
PAM configuration. The desktop and session determine which keyring service is
used; Fomalhaut does not depend on or prescribe a specific implementation. The
previous display manager's keyring configuration does not automatically apply
to greetd. Automatic unlocking is handled by the selected keyring service's PAM
module, while the current installer and AUR packages leave
`/etc/pam.d/greetd` unchanged.

### Common keyring implementations

First determine which keyring service your desktop session uses, then follow its
documentation:

| Keyring | Common environment | ArchWiki |
| --- | --- | --- |
| GNOME Keyring | GNOME and applications using the Secret Service API | [GNOME/Keyring](https://wiki.archlinux.org/title/GNOME/Keyring) |
| KDE Wallet | KDE Plasma | [KDE Wallet](https://wiki.archlinux.org/title/KDE_Wallet) |

KDE Wallet uses a different PAM module. Its implementation and configuration
parameters can change with the Plasma version, so do not apply the GNOME
Keyring module lines below directly to KWallet. Follow the ArchWiki,
distribution documentation, and your actual PAM configuration.

### GNOME Keyring example configuration (Arch Linux)

These steps apply to password logins using GNOME Keyring. Ensure that
`gnome-keyring` is installed and that the password for the `login` keyring
matches your account's login password. You can change the keyring password in
Seahorse (Passwords and Keys). Deleting keyring files to suppress prompts can
lose stored application credentials. Automatic login, fingerprint, and security
key authentication generally do not provide PAM with a login password for
decryption, so adding these modules alone cannot unlock the keyring for those
login methods.

Back up the existing configuration, then edit it:

```sh
sudo cp -a /etc/pam.d/greetd "/etc/pam.d/greetd.bak.$(date +%Y%m%d%H%M%S)"
sudoedit /etc/pam.d/greetd
```

Preserve the distribution's and administrator's existing authentication rules.
Add the `pam_gnome_keyring.so` `auth` line after the `auth` rules that obtain the
login password, and its `session` line after the existing `session` rules. If
the file or its included PAM configuration already provides equivalent rules,
avoid adding duplicates. For the default Arch configuration using
`system-local-login`, the merged file looks like this:

```text
#%PAM-1.0

auth       required     pam_securetty.so
auth       requisite    pam_nologin.so
auth       include      system-local-login
-auth      optional     pam_gnome_keyring.so
account    include      system-local-login
session    include      system-local-login
-session   optional     pam_gnome_keyring.so auto_start
```

`optional` means a keyring module failure by itself will not reject a normal
login. The `-` before the module type suppresses load errors when the module is
missing. These lines supplement the existing password authentication and
session rules. PAM stacks vary across distributions; merge according to your
distribution's documentation rather than replacing the whole file with this
example.

Saving the file requires no greetd restart. Save your work, log out normally,
and log back in through Fomalhaut using your login password. Editing the
configuration does not automatically unlock the keyring in an existing desktop
session. Open the application that previously triggered the prompt to confirm
that unlocking is no longer requested. To roll back, restore
`/etc/pam.d/greetd` from the actual backup file created above. Keep an
administrator terminal available while editing PAM until you have verified a
new login.

### Repeated GNOME Keyring unlock prompts after login

Automatic unlocking configures password delivery at login. If you successfully
unlock the keyring and receive another prompt in the same desktop session,
check whether the keyring service exited unexpectedly, restarted, or the
keyring was locked again:

```sh
systemctl --user show gnome-keyring-daemon.service -p ActiveState -p NRestarts
journalctl --user -b -u gnome-keyring-daemon.service
```

These commands apply to systems running GNOME Keyring as a systemd user
service. `NRestarts` counts automatic restarts of that service; it does not
include all possible manual restarts or other launch methods. A crash and
restart loses the previous process's in-memory unlock state, so later
credential requests may prompt again. PAM configuration alone cannot repair
that crash. Use the logs to investigate GNOME Keyring and its dependencies,
session startup, and applications requesting credentials. The prompt alone
does not establish a Fomalhaut authentication defect or a damaged keyring file.

This setup targets `/etc/pam.d/greetd` for greetd login.
`/etc/pam.d/fomalhaut-lock` handles reauthentication when unlocking the current
user's screen; changing it does not configure login.

References: [GNOME Keyring PAM module source](https://gitlab.gnome.org/GNOME/gnome-keyring/-/tree/main/pam),
[ArchWiki GNOME/Keyring](https://wiki.archlinux.org/title/GNOME/Keyring).

## Wayland locker, PAM, and readiness

Run `fomalhaut-lock` inside the logged-in user's own Wayland session. The
compositor must advertise `ext-session-lock-v1`. Unsupported compositors, X11,
and desktops that restrict third-party lockers fail explicitly before locking;
there is no fallback to an ordinary fullscreen or layer-shell window. Arch
installations also require `pam` and `gtk4-layer-shell` 1.2 or later; the source
installer checks and installs missing distribution packages.

The locker requires matching real, effective, saved, and filesystem UIDs, all
nonroot, then fixes the identity through NSS. Themes cannot submit usernames or
switch users. Each authentication uses a separate, single-use PAM subprocess
with the fixed policy `/etc/pam.d/fomalhaut-lock`. On a new Arch installation,
the installer provides:

```pam
auth      include  system-auth
account   include  system-auth
```

When AccountsService is available, the locker matches the current user object
using the fixed UID and NSS username through read-only calls. It displays the
avatar through the same secure in-memory proxy as the greeter. It does not use
the `[users]` enumeration policy. Service unavailability, mismatched identity
attributes, missing avatars, or failed file validation use the theme fallback
without affecting session locking or PAM authentication. AccountsService is
therefore optional for the locker.

Administrators should review this policy for their distribution and local
authentication requirements. Later source installations preserve a differing
policy and warn. If the PAM worker, renderer, theme, or controller fails while
locked, the locker retains the session lock and switches to a trusted GTK
failure/retry interface. Cancelling authentication also retains the lock.
Incorrect passwords, account policy rejections, and attempt limits are ordinary
PAM rejections: the current Web theme displays `auth.failed` and permits retries
without switching to the native failure page.

The source installer provides a `Type=notify` user service. After installation
or an update, reload the user manager and start the service:

```sh
systemctl --user daemon-reload
systemctl --user start fomalhaut-lock.service
```

The second command returns only after the compositor reports `locked`, the
controller records `lock.acquired`, and the locker sends `READY=1`. The process
exits normally after successful authentication and unlocking. The service is
started on demand by lock triggers and requires no enablement. Running
`fomalhaut-lock` directly also keeps it in the foreground, but without a
systemd readiness consumer.

The user unit explicitly sets `NoNewPrivileges=no`. This is a PAM compatibility
requirement; Fomalhaut itself runs as a regular user without a setuid bit. Arch's
`pam_unix` needs to execute the system's setuid `unix_chkpwd` helper.
`NoNewPrivileges=yes` would prevent that helper from acquiring the identity
needed to check the protected password database. In a systemd user scope,
`LockPersonality=yes` and `RestrictSUIDSGID=yes` also implicitly enable
`NoNewPrivs` to install seccomp filters, so the recommended unit leaves both
unset. Review this requirement when replacing the PAM stack. Restoring those
hardening settings requires moving PAM dispatch into a separate privilege
domain first; they cannot be enabled directly in the current embedded worker
architecture.

The unit also sets `UnsetEnvironment=GDK_SCALE GDK_DPI_SCALE` to clear toolkit
scaling variables inherited by the user manager for the locker service only.
niri/GTK still handle output scaling; extra page zoom comes solely from the
locker's configuration. When debugging by running the binary directly, check
that these variables are unset. Use the systemd user service for normal use.

niri users can merge the two snippets installed at
`/usr/local/share/doc/fomalhaut-lock/niri.kdl` into
`~/.config/niri/config.kdl`. The top-level startup entry handles idle and
suspend integration:

```kdl
spawn-at-startup "swayidle" "-w" "timeout" "300" "systemctl --user start fomalhaut-lock.service" "lock" "systemctl --user start fomalhaut-lock.service" "before-sleep" "systemctl --user start fomalhaut-lock.service"
```

Add a manual lock shortcut inside your existing `binds` block:

```kdl
Super+Alt+L hotkey-overlay-title="Lock the Screen: Fomalhaut" { spawn "systemctl" "--user" "start" "fomalhaut-lock.service"; }
```

Despite its name, `swayidle` uses standard Wayland idle/session protocols and
also works with other compositors such as niri. The standalone command example
installed at `/usr/local/share/doc/fomalhaut-lock/swayidle.conf` is:

```sh
swayidle -w \
    timeout 300 'systemctl --user start fomalhaut-lock.service' \
    lock 'systemctl --user start fomalhaut-lock.service' \
    before-sleep 'systemctl --user start fomalhaut-lock.service'
```

Keep the blocking `systemctl start` in `before-sleep` so suspend proceeds only
after readiness. Current automated tests cover readiness datagrams, role
protocols, the PAM worker's fail-closed state machine, and event routing across
views. Actual multi-output behavior, hotplug, suspend, and PAM module
combinations on niri and another compositor still require validation on target
systems.

## External theme configuration

```toml
[themes]
default = "my-theme"
```

The `default` theme applies to both greeter and locker. You can override each
role independently; selection follows “role override → `default` → embedded
minimal theme”:

```toml
[themes]
default = "nocturne"
greeter = "custom-greeter"
locker = "/srv/fomalhaut/themes/custom-locker"
```

Each field accepts a stable theme ID or an absolute path. Relative paths are
invalid. IDs are discovered in `/usr/local/share/fomalhaut/themes`, then
`/usr/share/fomalhaut/themes`. Duplicate IDs within the same root are selected
in directory path lexical order with a warning. Each theme still has one
`theme.toml` entrypoint; the same page uses the SDK's runtime mode to render
the greeter or locker. Both hosts use this selection policy.

The old `[frontend].path` setting is no longer supported. The runtime rejects
`[frontend]` as an unknown top-level field, and the source installer does not
migrate or remove it. Before upgrading an old installation, manually change
`path` to `[themes].default`. If installer preflight finds `[frontend]`, it
fails before switching binaries, themes, or other installed files.

Each theme directory directly under a discovery root, or selected by an
explicit absolute path, must contain `theme.toml`:

```toml
[theme]
id = "my-theme"
name = "My Theme"
protocol = 1
entrypoint = "index.html"
```

`id` is a stable lowercase ASCII kebab-case identity of 1–64 bytes. Preserve it
when changing the display name after publication; `name` is for display only.
Discovery excludes user home directories, XDG directories, network locations,
and arbitrary recursive paths. If a selected higher-priority match has a
broken manifest, protocol, or entrypoint, the host refuses to start rather than
silently choosing a lower-priority theme with the same name.

The entrypoint and other resources load through `fomalhaut://theme/`. Use
relative URLs in HTML:

```html
<link rel="stylesheet" href="style.css">
<script src="app.js"></script>
```

Themes cannot use inline scripts, remote URLs, network requests, frames,
objects, or form navigation. Resource paths support only ASCII letters,
digits, `-`, `_`, `.`, and `/`; percent-encoded filenames are unsupported.
Only the manifest entrypoint can be the top-level page; other HTML files cannot
be navigation targets.

The frontend sends protocol v1 requests through:

```js
const response = await window.webkit.messageHandlers.fomalhaut.postMessage({
  protocol: 1,
  id: 1,
  method: 'state.get',
  params: {},
});

window.addEventListener('fomalhaut:event', (event) => {
  console.log(event.detail);
});
```

See `protocol/v1.schema.json` in the repository root for complete request,
response, event, and length constraints. `state.get` returns a snapshot union
discriminated by `mode: "greeter" | "locker"`, with `locale: "en" | "zh-CN"`
and the last published event `sequence`.

Narrow by `mode` first. The greeter handles user and session selection and
starts authentication with `auth.begin(username)`. The locker displays only
the host's fixed `identity`, provides no user/session switching, and
reauthenticates the current user with parameterless `auth.begin()`. Both share
`auth.respond`, `auth.cancel`, prompts/messages, and common authentication
states. `session.*` belongs to the greeter; `lock.*` belongs to the locker.
Themes can read authentication input on their pages, so administrators should
install only trusted themes, and themes should clear input elements
synchronously before SDK calls.

Use the same `fomalhaut-sdk` for both roles. Its factory bootstraps the client
and returns a generic client narrowed by its read-only `mode` field:

```ts
const client = await createFomalhautClient();

if (client.mode === "greeter") {
  await client.auth.begin("alice");
  await client.session.select("wayland:sway");
} else {
  await client.auth.begin();
  // TypeScript narrows client.session to undefined here.
}
```

Build the React reference theme in the repository with:

```sh
bun run build:theme:nocturne
```

Build output is written to `themes/nocturne/dist`. For local repository testing,
use its absolute path directly:

```toml
[themes]
default = "/home/example/Projects/fomalhaut/themes/nocturne/dist"
```

The `greeter` user must be able to traverse and read this directory. The browser
development server uses a development-only mock transport that accepts
`fomalhaut` as the fixture password. Production builds exclude that transport
and reject login requests without the real WebKit bridge.

The greeter branch of `state.get` provides a host-filtered `users` array with
`username`, `displayName`, and optional `avatarUrl`. Avatar URLs always use the
host-managed `fomalhaut://avatar/<id>` scheme; themes must not derive local
paths from usernames. The user list may be empty, so themes must keep a manual
username input available. After selecting a user summary, pass its `username`
to `auth.begin`; greetd/PAM alone determines the authentication result. The
locker branch provides only the current trusted `identity` with the same
display fields, without UIDs, account enumeration, or permission for themes to
change the authentication target.

## Power management

When `[power]` is absent, runtime configuration disables all power actions.
When creating a configuration for the first time, the source installer
explicitly allows all three fixed actions:

```toml
[power]
actions = ["poweroff", "reboot", "suspend"]
```

The array accepts only these three values without duplicates. An explicitly
empty array disables all actions. Updates preserve an absent `[power]`, an
explicitly empty array, or a custom allowlist. Fomalhaut queries systemd-logind
through system D-Bus. `state.get` includes an action in `capabilities.power`
only if configuration allows it and the corresponding `Can*` method returns
`yes`. Responses of `no`, `na`, or `challenge`, or unavailable logind, exclude
the action, so neither role depends on an interactive Polkit agent.

Themes can request only advertised actions through `power.request`. The greeter
first cancels current greetd authentication; the locker cancels the current
PAM transaction. They then use the shared backend's noninteractive
`PowerOff(false)`, `Reboot(false)`, or `Suspend(false)` calls. They do not
execute shell commands or fall back to `systemctl`. Locker power requests do
not authorize unlocking or release the session lock; suspend/resume keeps the
session locked. The distribution's Polkit/logind policy must still allow the
running user to perform the action, or it remains hidden or returns a redacted
error.

## Page scaling

A separate Cage greeter session does not inherit KDE/GNOME display scaling.
Set fractional WebKit page scaling explicitly:

```toml
[display]
scale = 1.5
```

This scalar applies to both roles. To configure them separately, use dotted
keys and provide both values:

```toml
[display]
scale.greeter = 1.5
scale.locker = 1.0
```

Both default to `1.0`; each value must be between `0.5` and `4.0`. Scaling uses
WebKit's `zoom-level` for the entire theme without changing theme code or the
cursor drawn by Cage. Configure cursor size independently in the greetd
command, for example:

```toml
command = "dbus-run-session env XCURSOR_SIZE=48 cage -s -m last -d -- /usr/bin/fomalhaut"
```

When using `[display].scale`, leave `GDK_SCALE` and `GDK_DPI_SCALE` unset to
avoid combining toolkit scaling with page zoom. For a separate Cage greeter
and a niri locker with `150%` output scaling, typically use `1.5` and `1.0`
respectively.

## User discovery

These enumeration settings apply only to the greeter. The locker remains
bound to the current session user and cannot enumerate or switch accounts.

The default configuration is equivalent to:

```toml
[users]
provider = "auto"
```

Available values:

- `auto`: prefer AccountsService; use a fixed `/usr/bin/getent passwd` NSS
  query only when the service is unavailable as a whole. Explicit access
  denial, a successful empty result, or a result containing only invalid
  entries is respected without an NSS fallback.
- `accounts_service`: use AccountsService only, without NSS fallback.
- `nss`: use NSS only through the restricted `getent passwd` subprocess.
- `none`: disable user enumeration completely.

NSS results are filtered by `UID_MIN`/`UID_MAX` in `/etc/login.defs` and login
shell. AccountsService avatars are exposed only after checks for file type,
ownership/trusted directories, size, and PNG/JPEG/WebP contents. Discovery
failures produce an empty user list without preventing greeter startup or
manual login. Install and enable AccountsService for display names and
avatars; the NSS fallback supplies usernames only.

## Session search directories

Override the Wayland, X11, and relative `TryExec` search directories with:

```toml
[sessions]
wayland_dirs = [
  "/usr/local/share/wayland-sessions",
  "/usr/share/wayland-sessions",
]
x11_dirs = [
  "/usr/local/share/xsessions",
  "/usr/share/xsessions",
]
executable_search_paths = ["/usr/local/bin", "/usr/bin"]
```

Directory order determines priority. Missing fields inherit the defaults
above; explicitly empty arrays disable that directory type. All paths must be
absolute. If no safe, valid session is discovered, Fomalhaut refuses to start.

## greetd/Cage example

```toml
[terminal]
vt = 1

[default_session]
command = "dbus-run-session cage -s -m last -d -- /usr/local/bin/fomalhaut"
user = "greeter"
```

The `greeter` user must be able to traverse and read binary, configuration,
and theme directories. Run Fomalhaut as a regular user.

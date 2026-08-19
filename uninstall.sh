#!/usr/bin/env bash

set -Eeuo pipefail
IFS=$'\n\t'

prefix="/usr/local"
system_root="/"

color_reset=""
color_blue=""
color_green=""
color_cyan=""
color_yellow=""
color_red=""
color_error_reset=""
color_dim=""
if [[ -t 1 && -z "${NO_COLOR+x}" && "${TERM:-dumb}" != "dumb" ]]; then
  color_reset=$'\033[0m'
  color_blue=$'\033[34m'
  color_green=$'\033[32m'
  color_cyan=$'\033[36m'
  color_yellow=$'\033[33m'
  color_dim=$'\033[2m'
fi
if [[ -t 2 && -z "${NO_COLOR+x}" && "${TERM:-dumb}" != "dumb" ]]; then
  color_red=$'\033[31m'
  color_error_reset=$'\033[0m'
fi

log_title() {
  printf '%s==>%s %s\n' "$color_blue" "$color_reset" "$1"
}

log_success() {
  printf '%s✓%s %s\n' "$color_green" "$color_reset" "$1"
}

log_unchanged() {
  printf '%s=%s %s\n' "$color_cyan" "$color_reset" "$1"
}

log_warning() {
  printf '%s!%s %s\n' "$color_yellow" "$color_reset" "$1"
}

log_note() {
  printf '%s%s%s\n' "$color_dim" "$1" "$color_reset"
}

die() {
  printf '%suninstall.sh: error:%s %s\n' "$color_red" "$color_error_reset" "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Remove a complete Fomalhaut source installation.

Usage: ./uninstall.sh [options]

Options:
  --prefix PATH       Remove source artifacts below PATH (default: /usr/local).
  --system-root PATH  Operate on an isolated staging root without sudo or systemctl.
  -h, --help          Show this help.

Source binaries, the old locker user unit, integration examples, and verified
source-managed Nocturne releases are removed. Configuration and unrecognized
legacy theme directories are preserved by default. Deleting them always
requires an explicit interactive confirmation; non-interactive runs keep them.
Installed AUR packages are detected per role: theme selectors and the greetd
command are migrated to available AUR replacements, and an AUR-managed PAM
policy is never removed.
EOF
}

require_value() {
  local option="$1"
  local value="${2-}"
  [[ -n "$value" ]] || die "$option requires a value"
}

while (($# > 0)); do
  case "$1" in
    --prefix)
      require_value "$1" "${2-}"
      prefix="$2"
      shift 2
      ;;
    --system-root)
      require_value "$1" "${2-}"
      system_root="$2"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *) die "unknown option: $1" ;;
  esac
done

[[ "$EUID" -ne 0 ]] || die "run this script as a regular user; it invokes sudo only for system changes"
command -v python3 >/dev/null 2>&1 || die "python3 is required"

normalize_absolute_path() {
  python3 - "$1" <<'PY'
import posixpath
import sys

path = sys.argv[1]
if not path.startswith("/"):
    raise SystemExit(1)
print(posixpath.normpath(path))
PY
}

prefix="$(normalize_absolute_path "$prefix")" || die "--prefix must be an absolute path"
system_root="$(normalize_absolute_path "$system_root")" || die "--system-root must be an absolute path"
[[ "$prefix" != "/" ]] || die "--prefix must not be /"
if [[ "$prefix" == "/usr" && "$system_root" != "/" ]]; then
  die "--prefix /usr is ambiguous in --system-root mode"
fi

if [[ "$system_root" == "/" ]]; then
  [[ -f /etc/arch-release ]] || die "source installation uninstall is supported only on Arch Linux"
  command -v pacman >/dev/null 2>&1 || die "pacman is required to detect package-managed replacements"
  command -v sudo >/dev/null 2>&1 || die "sudo is required for system changes"
  privileged=(sudo)
else
  [[ -d "$system_root" ]] || die "--system-root must name an existing directory"
  system_root="$(cd -- "$system_root" && pwd -P)"
  privileged=()
fi

run_privileged() {
  "${privileged[@]}" "$@"
}

rooted() {
  local runtime_path="$1"
  if [[ "$system_root" == "/" ]]; then
    printf '%s\n' "$runtime_path"
  else
    printf '%s%s\n' "$system_root" "$runtime_path"
  fi
}

aur_greeter="$(rooted /usr/bin/fomalhaut)"
aur_locker="$(rooted /usr/bin/fomalhaut-lock)"
aur_locker_unit="$(rooted /usr/lib/systemd/user/fomalhaut-lock.service)"
aur_theme="$(rooted /usr/share/fomalhaut/themes/nocturne)"
aur_greeter_installed=false
aur_locker_installed=false
aur_theme_installed=false
if [[ "$system_root" == "/" ]]; then
  pacman -Q greetd-fomalhaut >/dev/null 2>&1 && aur_greeter_installed=true
  pacman -Q fomalhaut-lock >/dev/null 2>&1 && aur_locker_installed=true
  pacman -Q fomalhaut-theme-nocturne >/dev/null 2>&1 && aur_theme_installed=true
else
  [[ -e "$aur_greeter" || -L "$aur_greeter" ]] && aur_greeter_installed=true
  if [[ -e "$aur_locker" || -L "$aur_locker" \
    || -e "$aur_locker_unit" || -L "$aur_locker_unit" ]]; then
    aur_locker_installed=true
  fi
  [[ -f "$aur_theme/theme.toml" ]] && aur_theme_installed=true
fi

if $aur_greeter_installed; then
  [[ -x "$aur_greeter" ]] || die "installed AUR greeter replacement is missing: /usr/bin/fomalhaut"
fi
if $aur_locker_installed; then
  [[ -x "$aur_locker" ]] || die "installed AUR locker replacement is missing: /usr/bin/fomalhaut-lock"
  [[ -f "$aur_locker_unit" && ! -L "$aur_locker_unit" ]] \
    || die "installed AUR locker unit is missing or unsafe: /usr/lib/systemd/user/fomalhaut-lock.service"
fi
if $aur_theme_installed; then
  [[ -f "$aur_theme/theme.toml" && ! -L "$aur_theme/theme.toml" ]] \
    || die "installed AUR theme replacement is missing or unsafe: /usr/share/fomalhaut/themes/nocturne/theme.toml"
  python3 - "$aur_theme/theme.toml" <<'PY' \
    || die "installed AUR theme replacement is not a valid Nocturne theme"
from pathlib import Path
import sys
import tomllib

path = Path(sys.argv[1])
try:
    with path.open("rb") as source:
        manifest = tomllib.load(source)
except (OSError, tomllib.TOMLDecodeError) as error:
    raise SystemExit(f"invalid AUR theme manifest: {error}")
theme = manifest.get("theme")
if (
    not isinstance(theme, dict)
    or theme.get("id") != "nocturne"
    or theme.get("protocol") != 1
    or theme.get("entrypoint") != "index.html"
):
    raise SystemExit("AUR theme manifest does not describe Nocturne protocol 1")
entrypoint = path.parent / "index.html"
if entrypoint.is_symlink() or not entrypoint.is_file():
    raise SystemExit("AUR theme entrypoint is missing or unsafe")
PY
fi
if [[ "$prefix" == "/usr" ]] \
  && { $aur_greeter_installed || $aur_locker_installed || $aur_theme_installed; }; then
  die "--prefix /usr overlaps an installed AUR package"
fi

log_title "Fomalhaut source installation uninstall"
if $aur_greeter_installed; then
  log_success "Detected greetd-fomalhaut AUR takeover"
else
  log_note "greetd-fomalhaut is not installed; removing the source greeter without a replacement."
fi
if $aur_locker_installed; then
  log_success "Detected fomalhaut-lock AUR takeover"
else
  log_note "fomalhaut-lock is not installed; removing the source locker without a replacement."
fi
if $aur_theme_installed; then
  log_success "Detected fomalhaut-theme-nocturne AUR takeover"
else
  log_note "fomalhaut-theme-nocturne is not installed; removing the source theme without a replacement."
fi

purge_config=false
printf '%s\n' "The following configuration is preserved by default:"
printf '  %s\n' \
  "/etc/fomalhaut/config.toml and its installer backups" \
  "/etc/greetd/config.toml and its installer backups" \
  "/etc/fomalhaut/themes/nocturne legacy directories not owned by a verified source release"
if $aur_locker_installed; then
  printf '%s\n' "The AUR-managed /etc/pam.d/fomalhaut-lock is always preserved."
else
  printf '  %s\n' "/etc/pam.d/fomalhaut-lock and its .pacnew file"
fi
if [[ -t 0 ]]; then
  printf 'Delete the listed configuration and Nocturne theme? [y/N] '
  response=""
  if ! IFS= read -r response; then
    response=""
  fi
  case "$response" in
    y | Y | yes | YES | Yes) purge_config=true ;;
    *) log_unchanged "Preserving configuration and Nocturne theme" ;;
  esac
else
  log_unchanged "Non-interactive input; preserving configuration and Nocturne theme"
fi

fomalhaut_config="$(rooted /etc/fomalhaut/config.toml)"
greetd_config="$(rooted /etc/greetd/config.toml)"
pam_config="$(rooted /etc/pam.d/fomalhaut-lock)"
source_theme_runtime="$prefix/share/fomalhaut/themes/nocturne"
source_theme_path="$(rooted "$source_theme_runtime")"
source_theme_parent="${source_theme_path%/*}"
source_theme_release_base="$source_theme_parent/.nocturne-releases"
legacy_theme_path="$(rooted /etc/fomalhaut/themes/nocturne)"
legacy_theme_parent="${legacy_theme_path%/*}"
legacy_theme_release_base="$legacy_theme_parent/.nocturne-releases"

validate_removable_leaf() {
  local path="$1"
  if [[ -e "$path" && ! -f "$path" && ! -L "$path" ]]; then
    die "refusing to remove non-file source artifact: $path"
  fi
}

for source_artifact in \
  "$(rooted "$prefix/bin/fomalhaut")" \
  "$(rooted "$prefix/bin/fomalhaut-lock")" \
  "$(rooted "$prefix/lib/systemd/user/fomalhaut-lock.service")" \
  "$(rooted "$prefix/share/doc/fomalhaut-lock/niri.kdl")" \
  "$(rooted "$prefix/share/doc/fomalhaut-lock/swayidle.conf")"; do
  validate_removable_leaf "$source_artifact"
done

if $purge_config; then
  validate_removable_leaf "$fomalhaut_config"
  validate_removable_leaf "$greetd_config"
  if ! $aur_locker_installed; then
    validate_removable_leaf "$pam_config"
    validate_removable_leaf "$pam_config.pacnew"
  fi
fi

if [[ "$system_root" == "/" ]]; then
  sudo -v
fi

if ! $purge_config && $aur_theme_installed; then
  run_privileged python3 - "$fomalhaut_config" "$source_theme_runtime" \
    "/etc/fomalhaut/themes/nocturne" <<'PY'
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import time
import tomllib

path = Path(sys.argv[1])
old_selectors = set(sys.argv[2:])
if not path.exists():
    print(f"= Preserved absent Fomalhaut configuration: {path}")
    raise SystemExit(0)
if path.is_symlink() or not path.is_file():
    raise SystemExit(f"refusing to migrate non-regular Fomalhaut configuration: {path}")

try:
    old_text = path.read_text(encoding="utf-8")
    parsed = tomllib.loads(old_text)
except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
    raise SystemExit(f"refusing to migrate invalid Fomalhaut configuration at {path}: {error}")

themes = parsed.get("themes")
if themes is None:
    print(f"= Preserved Fomalhaut configuration without [themes]: {path}")
    raise SystemExit(0)
if not isinstance(themes, dict):
    raise SystemExit("refusing to migrate invalid [themes] configuration")
selected_keys = [
    key
    for key in ("default", "greeter", "locker")
    if themes.get(key) in old_selectors
]
if not selected_keys:
    print("= Fomalhaut theme selectors already avoid source installation paths")
    raise SystemExit(0)

lines = old_text.splitlines()
table_pattern = re.compile(r"^\s*\[([^\[\]]+)\]\s*(?:#.*)?$")
tables = [
    (match.group(1).strip(), index)
    for index, line in enumerate(lines)
    if (match := table_pattern.match(line))
]
starts = [index for name, index in tables if name == "themes"]
if len(starts) != 1:
    raise SystemExit("refusing to migrate duplicate or missing [themes]")
start = starts[0]
end = min((index for _, index in tables if index > start), default=len(lines))
for key in selected_keys:
    key_pattern = re.compile(rf"^\s*{re.escape(key)}\s*=")
    matches = [index for index in range(start + 1, end) if key_pattern.match(lines[index])]
    if len(matches) != 1:
        raise SystemExit(f"refusing to migrate duplicate or missing [themes].{key}")
    line_index = matches[0]
    indentation = lines[line_index][: len(lines[line_index]) - len(lines[line_index].lstrip())]
    lines[line_index] = f"{indentation}{key} = {json.dumps('nocturne')}"

new_text = "\n".join(lines).rstrip() + "\n"
try:
    verified = tomllib.loads(new_text)
except tomllib.TOMLDecodeError as error:
    raise SystemExit(f"generated Fomalhaut configuration is invalid: {error}")
for key in selected_keys:
    if verified.get("themes", {}).get(key) != "nocturne":
        raise SystemExit(f"generated configuration did not migrate [themes].{key}")

old_stat = path.stat()
stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
backup = path.with_name(f"{path.name}.bak.{stamp}.{os.getpid()}")
shutil.copy2(path, backup)
os.chown(backup, old_stat.st_uid, old_stat.st_gid)

descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
temporary = Path(temporary_name)
try:
    with os.fdopen(descriptor, "w", encoding="utf-8") as output:
        output.write(new_text)
        output.flush()
        os.fsync(output.fileno())
    os.chmod(temporary, old_stat.st_mode & 0o7777)
    os.chown(temporary, old_stat.st_uid, old_stat.st_gid)
    os.replace(temporary, path)
    directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
finally:
    if temporary.exists():
        temporary.unlink()

print(f"✓ Migrated Fomalhaut theme selectors to nocturne; backup: {backup}")
PY
elif ! $purge_config && ! $aur_theme_installed \
  && [[ -e "$fomalhaut_config" || -L "$fomalhaut_config" ]]; then
  log_warning "Preserved Fomalhaut configuration may reference the removed source theme"
fi

if ! $purge_config && $aur_greeter_installed; then
  run_privileged python3 - "$greetd_config" "$prefix/bin/fomalhaut" "/usr/bin/fomalhaut" <<'PY'
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import time
import tomllib

path = Path(sys.argv[1])
old_executable = sys.argv[2]
new_executable = sys.argv[3]
if not path.exists():
    print(f"= Preserved absent greetd configuration: {path}")
    raise SystemExit(0)
if path.is_symlink() or not path.is_file():
    raise SystemExit(f"refusing to migrate non-regular greetd configuration: {path}")

try:
    old_text = path.read_text(encoding="utf-8")
    parsed = tomllib.loads(old_text)
except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
    raise SystemExit(f"refusing to migrate invalid greetd configuration at {path}: {error}")

default_session = parsed.get("default_session")
if not isinstance(default_session, dict):
    print(f"= Preserved greetd configuration without [default_session]: {path}")
    raise SystemExit(0)
command = default_session.get("command")
if not isinstance(command, str):
    print(f"= Preserved greetd configuration without a string command: {path}")
    raise SystemExit(0)
token_pattern = re.compile(rf"(?<!\S){re.escape(old_executable)}(?!\S)")
matches_in_command = list(token_pattern.finditer(command))
if not matches_in_command:
    print(f"= Greetd command already avoids {old_executable}")
    raise SystemExit(0)
if len(matches_in_command) != 1:
    raise SystemExit("refusing to migrate an ambiguous greetd command")

lines = old_text.splitlines()
table_pattern = re.compile(r"^\s*\[([^\[\]]+)\]\s*(?:#.*)?$")
tables = [
    (match.group(1).strip(), index)
    for index, line in enumerate(lines)
    if (match := table_pattern.match(line))
]
starts = [index for name, index in tables if name == "default_session"]
if len(starts) != 1:
    raise SystemExit("refusing to migrate duplicate or missing [default_session]")
start = starts[0]
end = min((index for _, index in tables if index > start), default=len(lines))
key_pattern = re.compile(r"^\s*command\s*=")
matches = [index for index in range(start + 1, end) if key_pattern.match(lines[index])]
if len(matches) != 1:
    raise SystemExit("refusing to migrate duplicate or missing [default_session].command")

new_command = token_pattern.sub(new_executable, command, count=1)
line_index = matches[0]
indentation = lines[line_index][: len(lines[line_index]) - len(lines[line_index].lstrip())]
lines[line_index] = f"{indentation}command = {json.dumps(new_command, ensure_ascii=True)}"
new_text = "\n".join(lines).rstrip() + "\n"
try:
    verified = tomllib.loads(new_text)
except tomllib.TOMLDecodeError as error:
    raise SystemExit(f"generated greetd configuration is invalid: {error}")
if verified.get("default_session", {}).get("command") != new_command:
    raise SystemExit("generated greetd configuration did not preserve the migrated command")

old_stat = path.stat()
stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
backup = path.with_name(f"{path.name}.bak.{stamp}.{os.getpid()}")
shutil.copy2(path, backup)
os.chown(backup, old_stat.st_uid, old_stat.st_gid)

descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
temporary = Path(temporary_name)
try:
    with os.fdopen(descriptor, "w", encoding="utf-8") as output:
        output.write(new_text)
        output.flush()
        os.fsync(output.fileno())
    os.chmod(temporary, old_stat.st_mode & 0o7777)
    os.chown(temporary, old_stat.st_uid, old_stat.st_gid)
    os.replace(temporary, path)
    directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
finally:
    if temporary.exists():
        temporary.unlink()

print(f"✓ Migrated greetd command to {new_executable}; backup: {backup}")
PY
elif ! $purge_config && [[ -e "$greetd_config" || -L "$greetd_config" ]]; then
  log_warning "Preserved greetd configuration may still reference the removed $prefix/bin/fomalhaut"
fi

remove_leaf() {
  local path="$1"
  if [[ ! -e "$path" && ! -L "$path" ]]; then
    return
  fi
  validate_removable_leaf "$path"
  run_privileged rm -- "$path"
  log_success "Removed $path"
}

remove_backups() {
  local base="$1"
  local candidate
  shopt -s nullglob
  for candidate in "$base.bak."*; do
    remove_leaf "$candidate"
  done
  shopt -u nullglob
}

source_greeter="$(rooted "$prefix/bin/fomalhaut")"
source_locker="$(rooted "$prefix/bin/fomalhaut-lock")"
source_locker_unit="$(rooted "$prefix/lib/systemd/user/fomalhaut-lock.service")"
source_niri="$(rooted "$prefix/share/doc/fomalhaut-lock/niri.kdl")"
source_swayidle="$(rooted "$prefix/share/doc/fomalhaut-lock/swayidle.conf")"

for source_artifact in \
  "$source_greeter" "$source_locker" "$source_locker_unit" "$source_niri" "$source_swayidle"; do
  remove_leaf "$source_artifact"
  remove_backups "$source_artifact"
done

for possibly_empty in \
  "$(rooted "$prefix/share/doc/fomalhaut-lock")" \
  "$(rooted "$prefix/lib/systemd/user")"; do
  if [[ -d "$possibly_empty" && ! -L "$possibly_empty" ]]; then
    run_privileged rmdir --ignore-fail-on-non-empty -- "$possibly_empty"
  fi
done

remove_config_tree() {
  local path="$1"
  if [[ ! -e "$path" && ! -L "$path" ]]; then
    return
  fi
  run_privileged rm -rf -- "$path"
  log_success "Removed $path"
}

remove_verified_source_theme() {
  local path="$1"
  local release_base="$2"
  local target=""
  if [[ -L "$path" ]]; then
    target="$(run_privileged readlink -- "$path")"
    if [[ ! "$target" =~ ^\.nocturne-releases/[A-Za-z0-9._-]+$ ]]; then
      log_warning "Preserved unrecognized theme symlink: $path"
      return
    fi
    remove_config_tree "$path"
    remove_config_tree "$release_base"
  elif [[ -e "$path" || -e "$release_base" ]]; then
    log_warning "Preserved unrecognized source theme layout at $path"
  fi
}

remove_verified_source_theme "$source_theme_path" "$source_theme_release_base"
remove_verified_source_theme "$legacy_theme_path" "$legacy_theme_release_base"

if $purge_config; then
  remove_leaf "$fomalhaut_config"
  remove_backups "$fomalhaut_config"
  remove_leaf "$greetd_config"
  remove_backups "$greetd_config"
  remove_config_tree "$source_theme_path"
  remove_config_tree "$source_theme_release_base"
  remove_config_tree "$legacy_theme_path"
  remove_config_tree "$legacy_theme_release_base"
  if ! $aur_locker_installed; then
    remove_leaf "$pam_config"
    remove_leaf "$pam_config.pacnew"
  fi

  shopt -s nullglob
  for legacy_theme in "$source_theme_path.legacy."* "$legacy_theme_path.legacy."*; do
    remove_config_tree "$legacy_theme"
  done
  shopt -u nullglob

  for possibly_empty in \
    "$source_theme_parent" "${source_theme_parent%/*}" \
    "$legacy_theme_parent" "${legacy_theme_parent%/*}"; do
    if [[ -d "$possibly_empty" && ! -L "$possibly_empty" ]]; then
      run_privileged rmdir --ignore-fail-on-non-empty -- "$possibly_empty"
    fi
  done
  log_success "Removed confirmed source-install configuration"
else
  log_success "Preserved Fomalhaut configuration and unrecognized legacy theme directories"
fi

if [[ "$system_root" == "/" ]]; then
  if command -v systemctl >/dev/null 2>&1; then
    if systemctl --user daemon-reload; then
      log_success "Reloaded the user systemd manager"
    else
      log_warning "Could not reload the user systemd manager; run 'systemctl --user daemon-reload' from your session"
    fi
  else
    log_warning "systemctl is unavailable; reload the user systemd manager before starting the locker"
  fi
fi

if $aur_greeter_installed || $aur_locker_installed || $aur_theme_installed; then
  log_success "Source installation uninstall and detected AUR migration complete"
else
  log_success "Source installation uninstall complete"
fi
if $aur_locker_installed; then
  log_note "Review compositor configuration and migrate remaining $prefix/bin/fomalhaut-lock references to /usr/bin/fomalhaut-lock."
else
  log_note "Remove remaining $prefix/bin/fomalhaut-lock references from compositor configuration."
fi
log_note "Restart greetd manually only when it is safe to end the current greeter session."

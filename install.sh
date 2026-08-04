#!/usr/bin/env bash

set -Eeuo pipefail
IFS=$'\n\t'

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"

system_root="/"
prefix="/usr/local"
display_scale=""
cursor_size="48"
greeter_user="greeter"
restart_greetd=false

color_reset=""
color_bold=""
color_blue=""
color_green=""
color_cyan=""
color_yellow=""
color_red=""
color_error_reset=""
color_dim=""
styled_output=false
if [[ -t 1 && -z "${NO_COLOR+x}" && "${TERM:-dumb}" != "dumb" ]]; then
  styled_output=true
  color_reset=$'\033[0m'
  color_bold=$'\033[1m'
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
  printf '%s%s==>%s %s\n' "$color_bold" "$color_blue" "$color_reset" "$1"
}

log_step() {
  printf '%s::%s %s\n' "$color_blue" "$color_reset" "$1"
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

run_build_command() {
  if $styled_output; then
    "$@"
  else
    env NO_COLOR=1 CLICOLOR=0 CARGO_TERM_COLOR=never CI=1 "$@"
  fi
}

usage() {
  cat <<'EOF'
Build and install Fomalhaut and the Nocturne reference theme.

Usage: ./install.sh [options]

Options:
  --display-scale SCALE  Set [display].scale (0.5 through 4.0).
  --cursor-size SIZE     Set Cage XCURSOR_SIZE (default: 48).
  --greeter-user USER   Set greetd default_session.user (default: greeter).
  --prefix PATH          Install the binary below PATH/bin (default: /usr/local).
  --system-root PATH     Install into a staging root without sudo or restart.
  --restart              Restart greetd after a successful system installation.
  -h, --help             Show this help.

Existing TOML files are parsed, backed up, selectively updated, revalidated,
and atomically replaced. On Arch Linux, missing build and runtime packages are
installed with paru, yay, or sudo pacman in that order.
EOF
}

die() {
  printf '%sinstall.sh: error:%s %s\n' "$color_red" "$color_error_reset" "$*" >&2
  exit 1
}

require_value() {
  local option="$1"
  local value="${2-}"
  [[ -n "$value" ]] || die "$option requires a value"
}

while (($# > 0)); do
  case "$1" in
    --display-scale)
      require_value "$1" "${2-}"
      display_scale="$2"
      shift 2
      ;;
    --cursor-size)
      require_value "$1" "${2-}"
      cursor_size="$2"
      shift 2
      ;;
    --greeter-user)
      require_value "$1" "${2-}"
      greeter_user="$2"
      shift 2
      ;;
    --prefix)
      require_value "$1" "${2-}"
      prefix="${2%/}"
      shift 2
      ;;
    --system-root)
      require_value "$1" "${2-}"
      system_root="${2%/}"
      shift 2
      ;;
    --restart)
      restart_greetd=true
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *) die "unknown option: $1" ;;
  esac
done

[[ "$EUID" -ne 0 ]] || die "run this script as a regular user; it invokes sudo only for installation"
[[ "$prefix" == /* && "$prefix" != "/" ]] || die "--prefix must be an absolute path other than /"
[[ "$system_root" == /* ]] || die "--system-root must be an absolute path"
[[ "$greeter_user" =~ ^[a-z_][a-z0-9_-]*$ ]] || die "--greeter-user is not a safe account name"
[[ "$cursor_size" =~ ^[0-9]+$ ]] || die "--cursor-size must be an integer"
((cursor_size >= 16 && cursor_size <= 256)) || die "--cursor-size must be between 16 and 256"

log_title "Fomalhaut source installer"

install_arch_dependencies() {
  [[ -f /etc/arch-release ]] || die "automatic dependency installation is supported only on Arch Linux"
  command -v pacman >/dev/null 2>&1 || die "pacman is required on Arch Linux"
  command -v sudo >/dev/null 2>&1 || die "sudo is required to install dependencies"

  local package_manager
  if command -v paru >/dev/null 2>&1; then
    package_manager="paru"
  elif command -v yay >/dev/null 2>&1; then
    package_manager="yay"
  else
    package_manager="pacman"
  fi

  local -a required_packages=(
    base-devel
    cage
    dbus
    diffutils
    git
    glib2
    glibc
    greetd
    gtk4
    libgcc
    libsoup3
    python
    webkitgtk-6.0
  )

  local -a missing_packages=()
  local package
  for package in "${required_packages[@]}"; do
    if ! pacman -T "$package" >/dev/null 2>&1; then
      missing_packages+=("$package")
    fi
  done
  if ((${#missing_packages[@]} == 0)); then
    log_success "Arch dependencies satisfied (preferred installer: $package_manager)"
    return
  fi

  log_step "Installing missing dependencies with $package_manager"
  printf '  %s•%s %s\n' "$color_dim" "$color_reset" "${missing_packages[@]}"
  case "$package_manager" in
    paru) paru -S --needed "${missing_packages[@]}" ;;
    yay) yay -S --needed "${missing_packages[@]}" ;;
    pacman) sudo pacman -S --needed "${missing_packages[@]}" ;;
    *) die "internal package-manager selection is invalid" ;;
  esac

  hash -r
  for package in "${required_packages[@]}"; do
    pacman -T "$package" >/dev/null 2>&1 || die "dependency remains unsatisfied after installation: $package"
  done
}

if [[ "$system_root" == "/" ]]; then
  install_arch_dependencies
fi

for command in cargo bun python3 git install cp mv ln find grep chmod chown cat cmp diff readlink date env; do
  command -v "$command" >/dev/null 2>&1 || die "required command is unavailable after dependency setup: $command"
done

if [[ -n "$display_scale" ]]; then
  python3 - "$display_scale" <<'PY' || die "--display-scale must be a finite number from 0.5 through 4.0"
import math
import sys

try:
    value = float(sys.argv[1])
except ValueError:
    raise SystemExit(1)
if not math.isfinite(value) or not 0.5 <= value <= 4.0:
    raise SystemExit(1)
PY
fi

if [[ "$system_root" == "/" ]]; then
  command -v getent >/dev/null 2>&1 || die "getent is required to validate the greeter account"
  [[ -x /usr/bin/dbus-run-session ]] || die "/usr/bin/dbus-run-session is required by the greetd command"
  [[ -x /usr/bin/cage ]] || die "/usr/bin/cage is required by the greetd command"
  getent passwd "$greeter_user" >/dev/null || die "greeter account does not exist: $greeter_user"
  privileged=(sudo)
else
  mkdir -p -- "$system_root"
  system_root="$(cd -- "$system_root" && pwd -P)"
  privileged=()
  $restart_greetd && die "--restart cannot be used with --system-root"
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

update_toml() {
  local path="$1"
  shift
  ((${#@} % 4 == 0)) || die "internal TOML update arguments are malformed"

  run_privileged python3 - "$path" "$color_green" "$color_cyan" "$color_yellow" \
    "$color_reset" "$@" <<'PY'
import json
import math
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import time
import tomllib

path = Path(sys.argv[1])
success_color, unchanged_color, warning_color, color_reset = sys.argv[2:6]
raw_updates = sys.argv[6:]
if len(raw_updates) % 4:
    raise SystemExit("TOML update arguments must be groups of four")
if path.is_symlink():
    raise SystemExit(f"refusing to replace symbolic-link configuration: {path}")
if path.exists() and not path.is_file():
    raise SystemExit(f"refusing to replace non-regular configuration: {path}")

def typed_value(kind: str, raw: str):
    if kind == "string":
        return raw
    if kind == "integer":
        return int(raw)
    if kind == "float":
        value = float(raw)
        if not math.isfinite(value):
            raise ValueError("non-finite TOML float")
        return value
    raise ValueError(f"unsupported TOML value kind: {kind}")

def encoded_value(kind: str, raw: str) -> str:
    value = typed_value(kind, raw)
    if kind == "string":
        return json.dumps(value, ensure_ascii=True)
    return str(value)

updates = []
for index in range(0, len(raw_updates), 4):
    section, key, kind, raw = raw_updates[index:index + 4]
    if not re.fullmatch(r"[A-Za-z0-9_-]+", section):
        raise SystemExit(f"unsafe TOML section name: {section}")
    if not re.fullmatch(r"[A-Za-z0-9_-]+", key):
        raise SystemExit(f"unsafe TOML key name: {key}")
    updates.append((section, key, kind, raw))

try:
    old_text = path.read_text(encoding="utf-8") if path.exists() else ""
    tomllib.loads(old_text) if old_text.strip() else {}
except (tomllib.TOMLDecodeError, UnicodeDecodeError) as error:
    raise SystemExit(f"refusing to modify invalid TOML at {path}: {error}")

lines = old_text.splitlines()
table_pattern = re.compile(r"^\s*\[([^\[\]]+)\]\s*(?:#.*)?$")

def table_ranges(current_lines):
    tables = []
    for line_index, line in enumerate(current_lines):
        match = table_pattern.match(line)
        if match:
            tables.append((match.group(1).strip(), line_index))
    return tables

for section, key, kind, raw in updates:
    tables = table_ranges(lines)
    starts = [line_index for name, line_index in tables if name == section]
    if len(starts) > 1:
        raise SystemExit(f"refusing to modify duplicate TOML section [{section}]")
    rendered = f"{key} = {encoded_value(kind, raw)}"
    if not starts:
        if lines and lines[-1].strip():
            lines.append("")
        lines.extend((f"[{section}]", rendered))
        continue

    start = starts[0]
    later_starts = [line_index for _, line_index in tables if line_index > start]
    end = min(later_starts, default=len(lines))
    key_pattern = re.compile(rf"^\s*{re.escape(key)}\s*=")
    matches = [
        line_index for line_index in range(start + 1, end)
        if key_pattern.match(lines[line_index])
    ]
    if len(matches) > 1:
        raise SystemExit(f"refusing to modify duplicate key [{section}].{key}")
    if matches:
        indentation = lines[matches[0]][:len(lines[matches[0]]) - len(lines[matches[0]].lstrip())]
        lines[matches[0]] = indentation + rendered
    else:
        lines.insert(end, rendered)

new_text = "\n".join(lines).rstrip() + "\n"
try:
    parsed = tomllib.loads(new_text)
except tomllib.TOMLDecodeError as error:
    raise SystemExit(f"generated TOML is invalid for {path}: {error}")

for section, key, kind, raw in updates:
    expected = typed_value(kind, raw)
    if parsed.get(section, {}).get(key) != expected:
        raise SystemExit(f"generated TOML did not preserve [{section}].{key}")

if path.exists() and new_text == old_text:
    print(f"{unchanged_color}={color_reset} Unchanged {path}")
    raise SystemExit(0)

path.parent.mkdir(parents=True, exist_ok=True)
old_stat = path.stat() if path.exists() else None
if old_stat is not None:
    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    backup = path.with_name(f"{path.name}.bak.{stamp}.{os.getpid()}")
    shutil.copy2(path, backup)
    os.chown(backup, old_stat.st_uid, old_stat.st_gid)
    print(f"{warning_color}!{color_reset} Backed up {path} to {backup}")

descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
temporary = Path(temporary_name)
try:
    with os.fdopen(descriptor, "w", encoding="utf-8") as output:
        output.write(new_text)
        output.flush()
        os.fsync(output.fileno())
    if old_stat is None:
        os.chmod(temporary, 0o644)
    else:
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

print(f"{success_color}✓{color_reset} Updated {path}")
PY
}

preflight_toml() {
  run_privileged python3 - "$@" <<'PY'
from pathlib import Path
import sys
import tomllib

for raw_path in sys.argv[1:]:
    path = Path(raw_path)
    if path.is_symlink():
        raise SystemExit(f"refusing to replace symbolic-link configuration: {path}")
    if not path.exists():
        continue
    if not path.is_file():
        raise SystemExit(f"refusing to replace non-regular configuration: {path}")
    try:
        with path.open("rb") as source:
            tomllib.load(source)
    except (tomllib.TOMLDecodeError, UnicodeDecodeError, OSError) as error:
        raise SystemExit(f"refusing to modify invalid TOML at {path}: {error}")
PY
}

cd -- "$SCRIPT_DIR"
[[ -f Cargo.lock && -f bun.lock && -f package.json ]] || die "run from a complete Fomalhaut checkout"

log_step "Building the release binary"
run_build_command cargo build --release --locked -p fomalhaut

log_step "Installing frozen Bun dependencies"
run_build_command bun install --frozen-lockfile

log_step "Building the Nocturne theme"
run_build_command bun run build:theme

binary_source="$SCRIPT_DIR/target/release/fomalhaut"
theme_source="$SCRIPT_DIR/packages/fomalhaut-theme/dist"
[[ -x "$binary_source" ]] || die "release binary was not produced"
[[ -f "$theme_source/index.html" && -f "$theme_source/theme.toml" ]] || die "theme build is incomplete"
if find "$theme_source" -type l -print -quit | grep -q .; then
  die "theme build contains a symbolic link"
fi

if [[ "$system_root" == "/" ]]; then
  sudo -v
fi

fomalhaut_config="$(rooted /etc/fomalhaut/config.toml)"
greetd_config="$(rooted /etc/greetd/config.toml)"
preflight_toml "$fomalhaut_config" "$greetd_config"

install_id="$(date -u +%Y%m%dT%H%M%SZ)-$(git rev-parse --short=12 HEAD)-$$"
runtime_binary="$prefix/bin/fomalhaut"
binary_path="$(rooted "$runtime_binary")"
binary_directory="${binary_path%/*}"
binary_temporary="$binary_directory/.fomalhaut.new.$install_id"

if [[ -f "$binary_path" && ! -L "$binary_path" && -x "$binary_path" ]] \
  && run_privileged cmp -s -- "$binary_source" "$binary_path"; then
  log_unchanged "Unchanged $runtime_binary"
else
  run_privileged install -d -m 0755 -- "$binary_directory"
  if [[ -e "$binary_path" || -L "$binary_path" ]]; then
    run_privileged cp -a -- "$binary_path" "$binary_path.bak.$install_id"
  fi
  run_privileged install -m 0755 -- "$binary_source" "$binary_temporary"
  run_privileged mv -fT -- "$binary_temporary" "$binary_path"
  log_success "Installed $runtime_binary"
fi

readonly theme_runtime="/etc/fomalhaut/themes/nocturne"
theme_path="$(rooted "$theme_runtime")"
theme_parent="${theme_path%/*}"
release_base="$theme_parent/.nocturne-releases"
release_path="$release_base/$install_id"
theme_link_temporary="$theme_parent/.nocturne-link.$install_id"

theme_unchanged=false
if [[ -L "$theme_path" && -d "$theme_path" ]]; then
  theme_link_target="$(run_privileged readlink -- "$theme_path")"
  if [[ "$theme_link_target" =~ ^\.nocturne-releases/[A-Za-z0-9._-]+$ ]]; then
    if run_privileged diff -qr -- "$theme_source" "$theme_path" >/dev/null; then
      theme_unchanged=true
    else
      diff_status=$?
      ((diff_status == 1)) || die "the installed theme could not be compared safely"
    fi
  fi
fi

if $theme_unchanged; then
  log_unchanged "Unchanged $theme_runtime"
else
  run_privileged install -d -m 0755 -- "$theme_parent" "$release_base" "$release_path"
  run_privileged cp -a -- "$theme_source/." "$release_path/"
  if [[ "$system_root" == "/" ]]; then
    run_privileged chown -R root:root -- "$release_path"
  fi
  run_privileged find "$release_path" -type d -exec chmod 0755 {} +
  run_privileged find "$release_path" -type f -exec chmod 0644 {} +
  run_privileged ln -s -- ".nocturne-releases/$install_id" "$theme_link_temporary"

  if [[ -e "$theme_path" && ! -L "$theme_path" ]]; then
    legacy_path="$theme_path.legacy.$install_id"
    run_privileged mv -T -- "$theme_path" "$legacy_path"
    if ! run_privileged mv -T -- "$theme_link_temporary" "$theme_path"; then
      run_privileged mv -T -- "$legacy_path" "$theme_path"
      die "theme symlink switch failed; the previous directory was restored"
    fi
    log_warning "Preserved the previous theme directory at $legacy_path"
  else
    run_privileged mv -fT -- "$theme_link_temporary" "$theme_path"
  fi
  log_success "Installed theme release $theme_runtime"
fi

run_privileged install -d -m 0755 -- "${fomalhaut_config%/*}" "${greetd_config%/*}"

fomalhaut_updates=(frontend path string "$theme_runtime")
if [[ -n "$display_scale" ]]; then
  fomalhaut_updates+=(display scale float "$display_scale")
elif [[ ! -e "$fomalhaut_config" ]]; then
  fomalhaut_updates+=(display scale float "1.0")
fi
update_toml "$fomalhaut_config" "${fomalhaut_updates[@]}"

greetd_command="/usr/bin/dbus-run-session /usr/bin/env XCURSOR_SIZE=$cursor_size /usr/bin/cage -s -m last -d -- $runtime_binary"
greetd_updates=(default_session command string "$greetd_command" default_session user string "$greeter_user")
if [[ ! -e "$greetd_config" ]]; then
  greetd_updates=(terminal vt integer "1" "${greetd_updates[@]}")
fi
update_toml "$greetd_config" "${greetd_updates[@]}"

if $restart_greetd; then
  command -v systemctl >/dev/null 2>&1 || die "systemctl is required by --restart"
  run_privileged systemctl restart greetd
  log_success "Restarted greetd"
else
  log_success "Installation complete"
  log_note 'Run `sudo systemctl restart greetd` when it is safe to end the current greeter session.'
fi

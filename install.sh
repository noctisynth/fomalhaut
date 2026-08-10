#!/usr/bin/env bash

set -Eeuo pipefail
IFS=$'\n\t'

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"

system_root="/"
prefix="/usr/local"
display_scale=""
greeter_scale=""
locker_scale=""
language=""
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
  printf '%s%sBuild and install Fomalhaut and the Nocturne reference theme.%s\n\n' \
    "$color_bold" "$color_blue" "$color_reset"
  printf '%sUsage:%s %s./install.sh%s %s[options]%s\n\n' \
    "$color_bold" "$color_reset" "$color_green" "$color_reset" "$color_dim" "$color_reset"
  printf '%sOptions:%s\n' "$color_bold" "$color_reset"
  printf '  %s%-23s%s %s\n' "$color_cyan" '--display-scale SCALE' "$color_reset" \
    'Set one [display].scale for both roles (0.5 through 4.0).'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--greeter-scale SCALE' "$color_reset" \
    'Set the greeter scale; requires --locker-scale.'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--locker-scale SCALE' "$color_reset" \
    'Set the locker scale; requires --greeter-scale.'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--language LANGUAGE' "$color_reset" \
    'Set [locale].language to en or zh-CN.'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--cursor-size SIZE' "$color_reset" \
    'Set Cage XCURSOR_SIZE (default: 48).'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--greeter-user USER' "$color_reset" \
    'Set greetd default_session.user (default: greeter).'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--prefix PATH' "$color_reset" \
    'Install the binary below PATH/bin (default: /usr/local).'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--system-root PATH' "$color_reset" \
    'Install into a staging root without sudo or restart.'
  printf '  %s%-23s%s %s\n' "$color_cyan" '--restart' "$color_reset" \
    'Restart greetd after a successful system installation.'
  printf '  %s%-23s%s %s\n\n' "$color_cyan" '-h, --help' "$color_reset" \
    'Show this help.'
  printf '%sExisting TOML files are parsed, backed up, selectively updated, revalidated,%s\n' \
    "$color_dim" "$color_reset"
  printf '%sand atomically replaced. On Arch Linux, missing build and runtime packages are%s\n' \
    "$color_dim" "$color_reset"
  printf '%sinstalled with paru, yay, or sudo pacman in that order.%s\n' \
    "$color_dim" "$color_reset"
  printf '%sFresh Fomalhaut configurations enable poweroff, reboot, and suspend; existing%s\n' \
    "$color_dim" "$color_reset"
  printf '%spower policy is preserved during updates.%s\n' \
    "$color_dim" "$color_reset"
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
    --greeter-scale)
      require_value "$1" "${2-}"
      greeter_scale="$2"
      shift 2
      ;;
    --locker-scale)
      require_value "$1" "${2-}"
      locker_scale="$2"
      shift 2
      ;;
    --language)
      require_value "$1" "${2-}"
      language="$2"
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
[[ -z "$language" || "$language" == "en" || "$language" == "zh-CN" ]] \
  || die "--language must be en or zh-CN"
[[ "$cursor_size" =~ ^[0-9]+$ ]] || die "--cursor-size must be an integer"
((cursor_size >= 16 && cursor_size <= 256)) || die "--cursor-size must be between 16 and 256"
[[ -z "$display_scale" || ( -z "$greeter_scale" && -z "$locker_scale" ) ]] \
  || die "--display-scale cannot be combined with role-specific scale options"
[[ -z "$greeter_scale" && -z "$locker_scale" || -n "$greeter_scale" && -n "$locker_scale" ]] \
  || die "--greeter-scale and --locker-scale must be provided together"

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
    clang
    dbus
    diffutils
    git
    glib2
    glibc
    greetd
    gtk4
    gtk4-layer-shell
    libgcc
    libsoup3
    pam
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

validate_scale() {
  local option="$1"
  local raw="$2"
  [[ -n "$raw" ]] || return 0
  python3 - "$raw" <<'PY' || die "$option must be a finite number from 0.5 through 4.0"
import math
import sys

try:
    value = float(sys.argv[1])
except ValueError:
    raise SystemExit(1)
if not math.isfinite(value) or not 0.5 <= value <= 4.0:
    raise SystemExit(1)
PY
}

validate_scale "--display-scale" "$display_scale"
validate_scale "--greeter-scale" "$greeter_scale"
validate_scale "--locker-scale" "$locker_scale"

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
    if kind == "remove-table":
        return None
    if kind == "string":
        return raw
    if kind == "string-array":
        value = json.loads(raw)
        if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
            raise ValueError("string-array value must be an array of strings")
        return value
    if kind == "integer":
        return int(raw)
    if kind == "float":
        value = float(raw)
        if not math.isfinite(value):
            raise ValueError("non-finite TOML float")
        return value
    if kind == "display-scale-shared":
        value = float(raw)
        if not math.isfinite(value):
            raise ValueError("non-finite shared display scale")
        return value
    if kind == "display-scale-roles":
        values = raw.split(",")
        if len(values) != 2:
            raise ValueError("role display scale requires greeter and locker values")
        greeter, locker = map(float, values)
        if not math.isfinite(greeter) or not math.isfinite(locker):
            raise ValueError("non-finite role display scale")
        return {"greeter": greeter, "locker": locker}
    raise ValueError(f"unsupported TOML value kind: {kind}")

def encoded_value(kind: str, raw: str) -> str:
    value = typed_value(kind, raw)
    if kind in ("string", "string-array"):
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

def remove_table(current_lines, table_name, allowed_keys):
    tables = table_ranges(current_lines)
    starts = [line_index for name, line_index in tables if name == table_name]
    if len(starts) > 1:
        raise SystemExit(f"refusing to modify duplicate TOML section [{table_name}]")
    if not starts:
        return None
    start = starts[0]
    later_starts = [line_index for _, line_index in tables if line_index > start]
    end = min(later_starts, default=len(current_lines))
    assignment_pattern = re.compile(r"^\s*([A-Za-z0-9_.-]+)\s*=")
    assignments = []
    for line_index in range(start + 1, end):
        match = assignment_pattern.match(current_lines[line_index])
        if match:
            assignments.append(match.group(1))
    if set(assignments) != set(allowed_keys) or len(assignments) != len(allowed_keys):
        expected = ", ".join(allowed_keys)
        raise SystemExit(
            f"refusing to replace [{table_name}]: expected exactly {expected}"
        )
    del current_lines[start:end]
    return start

def update_display_scale(current_lines, kind, raw):
    removed_table_index = remove_table(
        current_lines, "display.scale", ("greeter", "locker")
    )
    tables = table_ranges(current_lines)
    starts = [line_index for name, line_index in tables if name == "display"]
    if len(starts) > 1:
        raise SystemExit("refusing to modify duplicate TOML section [display]")
    if not starts:
        if removed_table_index is None:
            if current_lines and current_lines[-1].strip():
                current_lines.append("")
            removed_table_index = len(current_lines)
        current_lines.insert(removed_table_index, "[display]")
        starts = [removed_table_index]

    start = starts[0]
    tables = table_ranges(current_lines)
    later_starts = [line_index for _, line_index in tables if line_index > start]
    end = min(later_starts, default=len(current_lines))
    scale_pattern = re.compile(r"^\s*scale(?:\.(?:greeter|locker))?\s*=")
    matches = [
        line_index for line_index in range(start + 1, end)
        if scale_pattern.match(current_lines[line_index])
    ]
    insertion_index = matches[0] if matches else None
    for line_index in reversed(matches):
        del current_lines[line_index]
    if insertion_index is not None:
        while insertion_index > start + 1 and not current_lines[insertion_index - 1].strip():
            del current_lines[insertion_index - 1]
            insertion_index -= 1
    if insertion_index is None:
        tables = table_ranges(current_lines)
        later_starts = [line_index for _, line_index in tables if line_index > start]
        insertion_index = min(later_starts, default=len(current_lines))
        while insertion_index > start + 1 and not current_lines[insertion_index - 1].strip():
            insertion_index -= 1
    value = typed_value(kind, raw)
    if kind == "display-scale-shared":
        rendered = [f"scale = {value}"]
    else:
        rendered = [
            f"scale.greeter = {value['greeter']}",
            f"scale.locker = {value['locker']}",
        ]
    needs_separator = (
        insertion_index < len(current_lines)
        and table_pattern.match(current_lines[insertion_index]) is not None
    )
    current_lines[insertion_index:insertion_index] = rendered
    if needs_separator:
        current_lines.insert(insertion_index + len(rendered), "")

for section, key, kind, raw in updates:
    if kind in ("display-scale-shared", "display-scale-roles"):
        if section != "display" or key != "scale":
            raise SystemExit("display scale updates must target [display].scale")
        update_display_scale(lines, kind, raw)
        continue
    tables = table_ranges(lines)
    starts = [line_index for name, line_index in tables if name == section]
    if len(starts) > 1:
        raise SystemExit(f"refusing to modify duplicate TOML section [{section}]")
    if kind == "remove-table":
        if not starts:
            continue
        start = starts[0]
        later_starts = [line_index for _, line_index in tables if line_index > start]
        end = min(later_starts, default=len(lines))
        assignments = []
        assignment_pattern = re.compile(r"^\s*([A-Za-z0-9_-]+)\s*=")
        for line_index in range(start + 1, end):
            match = assignment_pattern.match(lines[line_index])
            if match:
                assignments.append((match.group(1), line_index))
        if len(assignments) != 1 or assignments[0][0] != key:
            raise SystemExit(
                f"refusing to remove [{section}]: expected it to contain only {key}"
            )
        del lines[start:end]
        while start > 0 and start <= len(lines) and not lines[start - 1].strip():
            del lines[start - 1]
            start -= 1
        continue
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
    if kind == "remove-table":
        if section in parsed:
            raise SystemExit(f"generated TOML did not remove [{section}]")
        continue
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

log_step "Building the release binaries"
run_build_command cargo build --release --locked -p fomalhaut -p fomalhaut-lock

log_step "Installing frozen Bun dependencies"
run_build_command bun install --frozen-lockfile

log_step "Building the Nocturne theme"
run_build_command bun run build:theme

binary_source="$SCRIPT_DIR/target/release/fomalhaut"
lock_binary_source="$SCRIPT_DIR/target/release/fomalhaut-lock"
theme_source="$SCRIPT_DIR/packages/fomalhaut-theme/dist"
[[ -x "$binary_source" ]] || die "release binary was not produced"
[[ -x "$lock_binary_source" ]] || die "locker release binary was not produced"
[[ -f "$SCRIPT_DIR/packaging/pam/fomalhaut-lock" ]] || die "locker PAM policy is missing"
[[ -f "$SCRIPT_DIR/packaging/systemd/fomalhaut-lock.service.in" ]] \
  || die "locker systemd service template is missing"
[[ -f "$SCRIPT_DIR/packaging/idle/swayidle.conf" ]] \
  || die "locker swayidle example is missing"
[[ -f "$SCRIPT_DIR/packaging/niri/fomalhaut-lock.kdl" ]] \
  || die "locker niri example is missing"
[[ -f "$theme_source/index.html" && -f "$theme_source/theme.toml" ]] || die "theme build is incomplete"
if find "$theme_source" -type l -print -quit | grep -q .; then
  die "theme build contains a symbolic link"
fi

if [[ "$system_root" == "/" ]]; then
  sudo -v
fi

runtime_binary="$prefix/bin/fomalhaut"
runtime_lock_binary="$prefix/bin/fomalhaut-lock"
service_source="$SCRIPT_DIR/target/fomalhaut-lock.service"
python3 - "$SCRIPT_DIR/packaging/systemd/fomalhaut-lock.service.in" \
  "$service_source" "$runtime_lock_binary" <<'PY' \
  || die "locker systemd service could not be generated"
from pathlib import Path
import sys

template_path, output_path, executable = map(Path, sys.argv[1:])
template = template_path.read_text(encoding="utf-8")
if template.count("@FOMALHAUT_LOCK@") != 1:
    raise SystemExit("service template must contain one executable placeholder")
if not executable.is_absolute():
    raise SystemExit("locker executable path must be absolute")
rendered = template.replace("@FOMALHAUT_LOCK@", str(executable))
Path(output_path).write_text(rendered, encoding="utf-8")
PY

fomalhaut_config="$(rooted /etc/fomalhaut/config.toml)"
greetd_config="$(rooted /etc/greetd/config.toml)"
pam_config="$(rooted /etc/pam.d/fomalhaut-lock)"
service_config="$(rooted "$prefix/lib/systemd/user/fomalhaut-lock.service")"
swayidle_path="$(rooted "$prefix/share/doc/fomalhaut-lock/swayidle.conf")"
niri_path="$(rooted "$prefix/share/doc/fomalhaut-lock/niri.kdl")"
preflight_toml "$fomalhaut_config" "$greetd_config"
for managed_file in "$pam_config" "$service_config" "$swayidle_path" "$niri_path"; do
  if [[ -L "$managed_file" || ( -e "$managed_file" && ! -f "$managed_file" ) ]]; then
    die "refusing to install over non-regular managed file: $managed_file"
  fi
done

install_id="$(date -u +%Y%m%dT%H%M%SZ)-$(git rev-parse --short=12 HEAD)-$$"
binary_path="$(rooted "$runtime_binary")"
lock_binary_path="$(rooted "$runtime_lock_binary")"
binary_directory="${binary_path%/*}"
binary_temporary="$binary_directory/.fomalhaut.new.$install_id"
lock_binary_temporary="$binary_directory/.fomalhaut-lock.new.$install_id"

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

if [[ -f "$lock_binary_path" && ! -L "$lock_binary_path" && -x "$lock_binary_path" ]] \
  && run_privileged cmp -s -- "$lock_binary_source" "$lock_binary_path"; then
  log_unchanged "Unchanged $runtime_lock_binary"
else
  run_privileged install -d -m 0755 -- "$binary_directory"
  if [[ -e "$lock_binary_path" || -L "$lock_binary_path" ]]; then
    run_privileged cp -a -- "$lock_binary_path" "$lock_binary_path.bak.$install_id"
  fi
  run_privileged install -m 0755 -- "$lock_binary_source" "$lock_binary_temporary"
  run_privileged mv -fT -- "$lock_binary_temporary" "$lock_binary_path"
  log_success "Installed $runtime_lock_binary"
fi

if [[ -f "$service_config" ]] \
  && run_privileged cmp -s -- "$service_source" "$service_config"; then
  log_unchanged "Unchanged $prefix/lib/systemd/user/fomalhaut-lock.service"
else
  run_privileged install -d -m 0755 -- "${service_config%/*}"
  if [[ -f "$service_config" ]]; then
    run_privileged cp -a -- "$service_config" "$service_config.bak.$install_id"
  fi
  service_temporary="${service_config%/*}/.fomalhaut-lock.service.new.$install_id"
  run_privileged install -m 0644 -- "$service_source" "$service_temporary"
  run_privileged mv -fT -- "$service_temporary" "$service_config"
  log_success "Installed $prefix/lib/systemd/user/fomalhaut-lock.service"
fi

swayidle_source="$SCRIPT_DIR/packaging/idle/swayidle.conf"
run_privileged install -d -m 0755 -- "${swayidle_path%/*}"
if [[ -f "$swayidle_path" ]] \
  && run_privileged cmp -s -- "$swayidle_source" "$swayidle_path"; then
  log_unchanged "Unchanged $prefix/share/doc/fomalhaut-lock/swayidle.conf"
else
  run_privileged install -m 0644 -- "$swayidle_source" "$swayidle_path"
  log_success "Installed $prefix/share/doc/fomalhaut-lock/swayidle.conf"
fi

niri_source="$SCRIPT_DIR/packaging/niri/fomalhaut-lock.kdl"
if [[ -f "$niri_path" ]] \
  && run_privileged cmp -s -- "$niri_source" "$niri_path"; then
  log_unchanged "Unchanged $prefix/share/doc/fomalhaut-lock/niri.kdl"
else
  run_privileged install -m 0644 -- "$niri_source" "$niri_path"
  log_success "Installed $prefix/share/doc/fomalhaut-lock/niri.kdl"
fi

pam_source="$SCRIPT_DIR/packaging/pam/fomalhaut-lock"
if [[ -f "$pam_config" ]]; then
  if run_privileged cmp -s -- "$pam_source" "$pam_config"; then
    log_unchanged "Unchanged /etc/pam.d/fomalhaut-lock"
  else
    log_warning "Preserved administrator PAM policy at /etc/pam.d/fomalhaut-lock"
  fi
else
  run_privileged install -d -m 0755 -- "${pam_config%/*}"
  run_privileged install -m 0644 -- "$pam_source" "$pam_config"
  log_success "Installed /etc/pam.d/fomalhaut-lock"
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

fomalhaut_updates=(frontend path remove-table "" themes default string "$theme_runtime")
if [[ -n "$display_scale" ]]; then
  fomalhaut_updates+=(display scale display-scale-shared "$display_scale")
elif [[ -n "$greeter_scale" ]]; then
  fomalhaut_updates+=(display scale display-scale-roles "$greeter_scale,$locker_scale")
elif [[ ! -e "$fomalhaut_config" ]]; then
  fomalhaut_updates+=(display scale display-scale-shared "1.0")
fi
if [[ -n "$language" ]]; then
  fomalhaut_updates+=(locale language string "$language")
fi
if [[ ! -e "$fomalhaut_config" ]]; then
  fomalhaut_updates+=(power actions string-array '["poweroff", "reboot", "suspend"]')
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

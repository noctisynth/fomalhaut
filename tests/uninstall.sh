#!/usr/bin/env bash

set -euo pipefail
IFS=$'\n\t'

readonly REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly UNINSTALLER="$REPOSITORY_ROOT/uninstall.sh"
temporary_directory="$(mktemp -d /tmp/fomalhaut-uninstall-test.XXXXXXXX)"
trap 'rm -rf -- "$temporary_directory"' EXIT

fail() {
  printf 'uninstall test failed: %s\n' "$*" >&2
  exit 1
}

assert_exists() {
  [[ -e "$1" || -L "$1" ]] || fail "expected path to exist: $1"
}

assert_missing() {
  [[ ! -e "$1" && ! -L "$1" ]] || fail "expected path to be absent: $1"
}

assert_contains() {
  grep -Fq -- "$2" "$1" || fail "expected $1 to contain: $2"
}

seed_root() {
  local root="$1"
  install -d \
    "$root/usr/local/bin" \
    "$root/usr/local/lib/systemd/user" \
    "$root/usr/local/share/doc/fomalhaut-lock" \
    "$root/etc/fomalhaut/themes/.nocturne-releases/release-test/assets" \
    "$root/etc/greetd" \
    "$root/etc/pam.d"

  install -m 0755 /dev/null "$root/usr/local/bin/fomalhaut"
  install -m 0755 /dev/null "$root/usr/local/bin/fomalhaut-lock"
  install -m 0755 /dev/null "$root/usr/local/bin/fomalhaut.bak.test"
  install -m 0644 /dev/null "$root/usr/local/lib/systemd/user/fomalhaut-lock.service"
  install -m 0644 /dev/null "$root/usr/local/lib/systemd/user/fomalhaut-lock.service.bak.test"
  install -m 0644 /dev/null "$root/usr/local/share/doc/fomalhaut-lock/niri.kdl"
  install -m 0644 /dev/null "$root/usr/local/share/doc/fomalhaut-lock/swayidle.conf"
  install -m 0644 /dev/null "$root/etc/pam.d/fomalhaut-lock"
  install -m 0644 /dev/null "$root/etc/pam.d/fomalhaut-lock.pacnew"

  printf '%s\n' \
    '[themes]' \
    'default = "/etc/fomalhaut/themes/nocturne"' \
    >"$root/etc/fomalhaut/config.toml"
  install -m 0644 "$root/etc/fomalhaut/config.toml" \
    "$root/etc/fomalhaut/config.toml.bak.existing"
  printf '%s\n' \
    '[terminal]' \
    'vt = 1' \
    '' \
    '[default_session]' \
    'command = "/usr/bin/dbus-run-session /usr/bin/cage -- /usr/local/bin/fomalhaut"' \
    'user = "greeter"' \
    >"$root/etc/greetd/config.toml"
  install -m 0644 "$root/etc/greetd/config.toml" \
    "$root/etc/greetd/config.toml.bak.existing"
  printf '%s\n' '<!doctype html>' \
    >"$root/etc/fomalhaut/themes/.nocturne-releases/release-test/index.html"
  printf '%s\n' 'asset' \
    >"$root/etc/fomalhaut/themes/.nocturne-releases/release-test/assets/theme.css"
  ln -s '.nocturne-releases/release-test' "$root/etc/fomalhaut/themes/nocturne"
  install -d "$root/etc/fomalhaut/themes/nocturne.legacy.test"
  printf '%s\n' 'legacy' >"$root/etc/fomalhaut/themes/nocturne.legacy.test/index.html"
}

add_aur_greeter() {
  local root="$1"
  install -Dm755 /dev/null "$root/usr/bin/fomalhaut"
}

add_aur_locker() {
  local root="$1"
  install -Dm755 /dev/null "$root/usr/bin/fomalhaut-lock"
  install -Dm644 /dev/null "$root/usr/lib/systemd/user/fomalhaut-lock.service"
}

test_preserves_and_migrates_configuration() {
  local root="$temporary_directory/preserve"
  local output="$temporary_directory/preserve.output"
  install -d "$root"
  seed_root "$root"
  add_aur_greeter "$root"
  add_aur_locker "$root"

  env NO_COLOR=1 "$UNINSTALLER" --system-root "$root" </dev/null >"$output"

  assert_missing "$root/usr/local/bin/fomalhaut"
  assert_missing "$root/usr/local/bin/fomalhaut.bak.test"
  assert_missing "$root/usr/local/bin/fomalhaut-lock"
  assert_missing "$root/usr/local/lib/systemd/user/fomalhaut-lock.service"
  assert_missing "$root/usr/local/share/doc/fomalhaut-lock/niri.kdl"
  assert_missing "$root/usr/local/share/doc/fomalhaut-lock/swayidle.conf"
  assert_exists "$root/usr/bin/fomalhaut"
  assert_exists "$root/usr/bin/fomalhaut-lock"
  assert_exists "$root/etc/pam.d/fomalhaut-lock"
  assert_exists "$root/etc/pam.d/fomalhaut-lock.pacnew"
  assert_exists "$root/etc/fomalhaut/config.toml"
  assert_exists "$root/etc/fomalhaut/config.toml.bak.existing"
  assert_exists "$root/etc/greetd/config.toml"
  assert_exists "$root/etc/greetd/config.toml.bak.existing"
  assert_exists "$root/etc/fomalhaut/themes/nocturne"
  assert_exists "$root/etc/fomalhaut/themes/nocturne.legacy.test"
  assert_contains "$root/etc/greetd/config.toml" '/usr/bin/fomalhaut'
  if grep -Fq -- '/usr/local/bin/fomalhaut' "$root/etc/greetd/config.toml"; then
    fail "preserved greetd configuration still references the source greeter"
  fi
  compgen -G "$root/etc/greetd/config.toml.bak.20*" >/dev/null \
    || fail "greetd migration did not create a backup"
  assert_contains "$output" 'Non-interactive input; preserving configuration'

  env NO_COLOR=1 "$UNINSTALLER" --system-root "$root" </dev/null \
    >"$temporary_directory/preserve-second.output"
  assert_contains "$root/etc/greetd/config.toml" '/usr/bin/fomalhaut'
}

test_purges_only_confirmed_configuration() {
  local root="$temporary_directory/purge"
  local output="$temporary_directory/purge.output"
  install -d "$root"
  seed_root "$root"
  add_aur_greeter "$root"
  add_aur_locker "$root"

  printf 'y\n' | script -qefc \
    "env NO_COLOR=1 '$UNINSTALLER' --system-root '$root'" /dev/null >"$output"

  assert_missing "$root/usr/local/bin/fomalhaut"
  assert_missing "$root/usr/local/bin/fomalhaut-lock"
  assert_missing "$root/etc/fomalhaut/config.toml"
  assert_missing "$root/etc/fomalhaut/config.toml.bak.existing"
  assert_missing "$root/etc/greetd/config.toml"
  assert_missing "$root/etc/greetd/config.toml.bak.existing"
  assert_missing "$root/etc/fomalhaut/themes/nocturne"
  assert_missing "$root/etc/fomalhaut/themes/.nocturne-releases"
  assert_missing "$root/etc/fomalhaut/themes/nocturne.legacy.test"
  assert_exists "$root/etc/pam.d/fomalhaut-lock"
  assert_exists "$root/etc/pam.d/fomalhaut-lock.pacnew"
  assert_exists "$root/usr/bin/fomalhaut"
  assert_exists "$root/usr/bin/fomalhaut-lock"
  assert_contains "$output" 'Removed confirmed source-install configuration'
}

test_plain_uninstall_preserves_configuration() {
  local root="$temporary_directory/plain-preserve"
  local output="$temporary_directory/plain-preserve.output"
  install -d "$root"
  seed_root "$root"

  env NO_COLOR=1 "$UNINSTALLER" --system-root "$root" </dev/null >"$output"

  assert_missing "$root/usr/local/bin/fomalhaut"
  assert_missing "$root/usr/local/bin/fomalhaut-lock"
  assert_exists "$root/etc/fomalhaut/config.toml"
  assert_exists "$root/etc/greetd/config.toml"
  assert_exists "$root/etc/fomalhaut/themes/nocturne"
  assert_exists "$root/etc/pam.d/fomalhaut-lock"
  assert_exists "$root/etc/pam.d/fomalhaut-lock.pacnew"
  assert_contains "$root/etc/greetd/config.toml" '/usr/local/bin/fomalhaut'
  assert_contains "$output" 'removing the source greeter without a replacement'
  assert_contains "$output" 'may still reference the removed /usr/local/bin/fomalhaut'
}

test_plain_uninstall_purges_confirmed_configuration() {
  local root="$temporary_directory/plain-purge"
  local output="$temporary_directory/plain-purge.output"
  install -d "$root"
  seed_root "$root"

  printf 'y\n' | script -qefc \
    "env NO_COLOR=1 '$UNINSTALLER' --system-root '$root'" /dev/null >"$output"

  assert_missing "$root/usr/local/bin/fomalhaut"
  assert_missing "$root/usr/local/bin/fomalhaut-lock"
  assert_missing "$root/etc/fomalhaut/config.toml"
  assert_missing "$root/etc/greetd/config.toml"
  assert_missing "$root/etc/fomalhaut/themes/nocturne"
  assert_missing "$root/etc/pam.d/fomalhaut-lock"
  assert_missing "$root/etc/pam.d/fomalhaut-lock.pacnew"
  assert_contains "$output" 'Source installation uninstall complete'
}

test_partial_aur_takeover_is_role_specific() {
  local greeter_root="$temporary_directory/greeter-only"
  local locker_root="$temporary_directory/locker-only"
  install -d "$greeter_root" "$locker_root"
  seed_root "$greeter_root"
  seed_root "$locker_root"
  add_aur_greeter "$greeter_root"
  add_aur_locker "$locker_root"

  env NO_COLOR=1 "$UNINSTALLER" --system-root "$greeter_root" </dev/null \
    >"$temporary_directory/greeter-only.output"
  assert_contains "$greeter_root/etc/greetd/config.toml" '/usr/bin/fomalhaut'
  assert_exists "$greeter_root/etc/pam.d/fomalhaut-lock"

  printf 'y\n' | script -qefc \
    "env NO_COLOR=1 '$UNINSTALLER' --system-root '$locker_root'" /dev/null \
    >"$temporary_directory/locker-only.output"
  assert_missing "$locker_root/etc/greetd/config.toml"
  assert_exists "$locker_root/etc/pam.d/fomalhaut-lock"
  assert_exists "$locker_root/etc/pam.d/fomalhaut-lock.pacnew"
}

test_missing_aur_replacement_fails_before_removal() {
  local root="$temporary_directory/missing-replacement"
  local output="$temporary_directory/missing-replacement.output"
  install -d "$root"
  seed_root "$root"
  add_aur_locker "$root"
  rm -- "$root/usr/bin/fomalhaut-lock"

  if env NO_COLOR=1 "$UNINSTALLER" --system-root "$root" </dev/null >"$output" 2>&1; then
    fail "missing AUR locker replacement unexpectedly succeeded"
  fi
  assert_exists "$root/usr/local/bin/fomalhaut"
  assert_exists "$root/usr/local/bin/fomalhaut-lock"
  assert_contains "$output" 'installed AUR locker replacement is missing'
}

test_invalid_greetd_configuration_fails_before_removal() {
  local root="$temporary_directory/invalid-config"
  local output="$temporary_directory/invalid-config.output"
  install -d "$root"
  seed_root "$root"
  add_aur_greeter "$root"
  printf '%s\n' '[default_session' >"$root/etc/greetd/config.toml"

  if env NO_COLOR=1 "$UNINSTALLER" --system-root "$root" </dev/null >"$output" 2>&1; then
    fail "invalid greetd configuration unexpectedly succeeded"
  fi
  assert_exists "$root/usr/local/bin/fomalhaut"
  assert_exists "$root/usr/local/bin/fomalhaut-lock"
  assert_contains "$output" 'refusing to migrate invalid greetd configuration'
}

test_preserves_and_migrates_configuration
test_purges_only_confirmed_configuration
test_plain_uninstall_preserves_configuration
test_plain_uninstall_purges_confirmed_configuration
test_partial_aur_takeover_is_role_specific
test_missing_aur_replacement_fails_before_removal
test_invalid_greetd_configuration_fails_before_removal
printf '%s\n' 'source uninstall and AUR migration tests passed'

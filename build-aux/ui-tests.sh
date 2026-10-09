#!/usr/bin/env bash
# Play every scenario in crates/money-manager/tests/ui/ into the real
# application and fail if one of them does not come out the way it says it
# should.
#
#   build-aux/ui-tests.sh                 on the display you are sitting at
#   build-aux/ui-tests.sh --headless      under a private headless sway
#   build-aux/ui-tests.sh [--headless] phone new-expense   only these
#
# A scenario is a MONEY_MANAGER_SCRIPT (crates/money-manager/src/script.rs)
# kept in a file, one step per line, with what it needs said in its header:
#
#   # mode: demo | fresh | legacy      the sample ledger, or an installation with none
#   # size: 1240x800          the window it was written against
#
# It passes when the application reaches the script's `quit` and exits with
# status 0: an `expect:` step that does not hold exits with 1, a crash exits
# with something else, and a script that never gets to its end runs into the
# timeout.
#
# Clicks are coordinates inside the window, so they only mean something at
# the size the scenario names. A tiling compositor gives a window the size it
# likes, not the one asked for; that is what --headless is for, and it is how
# CI runs these. Without it, scenarios that click may fail on your desktop
# while the keyboard-only ones still hold.
#
# Every run gets its own empty XDG directories, so a scenario never sees your
# ledger or your settings, and no scenario sees what another left. The day is
# pinned too: the sample ledger is dated relative to today, and a scenario
# that states how many records there are has to mean one particular day.
set -Eeuo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$root"

scenarios_dir=crates/money-manager/tests/ui
app=${MONEY_MANAGER_BIN:-target/debug/money-manager}
timeout_s=${UI_TEST_TIMEOUT:-90}
today=${UI_TEST_TODAY:-2026-10-05}

headless=false
if [[ ${1:-} == --headless ]]; then
  shift
  headless=true
  runtime=$(mktemp -d)
  chmod 700 -- "$runtime"
  export XDG_RUNTIME_DIR=$runtime
  unset DISPLAY WAYLAND_DISPLAY
  # sway on wlroots' headless backend. It tiles, which here is the point: with
  # no borders and no bar the one window is exactly as large as the output,
  # and the output can be resized per scenario. (weston's headless backend
  # has no wl_seat at all, and the toolkit will not start without one.)
  # pixman, so the compositor needs no GPU; the application draws itself
  # through whatever Vulkan driver there is — lavapipe in CI.
  # --unsupported-gpu: sway refuses to start when the kernel has Nvidia's
  # module loaded, and a container sees its host's modules. Nothing here
  # touches the GPU, so the refusal is about a driver that is never used.
  printf '%s\n' 'default_border none' 'default_floating_border none' \
    'focus_follows_mouse no' > "$runtime/sway.conf"
  WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
    sway --unsupported-gpu -c "$runtime/sway.conf" >"$runtime/sway.log" 2>&1 &
  compositor_pid=$!
  for _ in $(seq 100); do
    socket=$(find "$runtime" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -print -quit)
    [[ -n $socket ]] && break
    sleep 0.1
  done
  [[ -n ${socket:-} ]] || { cat "$runtime/sway.log" >&2; exit 1; }
  export WAYLAND_DISPLAY=${socket##*/}
  SWAYSOCK=$(find "$runtime" -maxdepth 1 -name 'sway-ipc.*' -print -quit)
  export SWAYSOCK
fi

[[ -x $app ]] || cargo build -p money-manager

if (( $# )); then
  files=()
  for name in "$@"; do files+=("$scenarios_dir/$name.script"); done
else
  files=("$scenarios_dir"/*.script)
fi

work=$(mktemp -d)
cleanup() {
  [[ -z ${compositor_pid:-} ]] || kill "$compositor_pid" 2>/dev/null || true
  [[ -z ${registry_pid:-} ]] || kill "$registry_pid" 2>/dev/null || true
  rm -rf -- "$work" ${runtime:+"$runtime"}
}
trap cleanup EXIT

# A stand-in for the Zed editor's extension registry, which the settings
# dialog installs themes from: a scenario must not depend on somebody else's
# service, or on there being a network at all. It is the two addresses the
# application asks, as files under a plain web server — the list, and one
# extension, "sample", packaged the way the registry packages them out of
# tests/ui/registry/. (The list is an index.html because that is what the
# server answers a directory with; the application reads it as what it is.)
# Without python3 there is no stand-in and the scenario that installs a theme
# fails; the others do not ask.
registry_port=${UI_TEST_REGISTRY_PORT:-18765}
if command -v python3 >/dev/null; then
  registry=$work/registry
  mkdir -p -- "$registry/extensions/sample"
  cat > "$registry/extensions/index.html" <<'JSON'
{"data": [
  {"id": "sample", "name": "Sample", "description": "A theme written for the tests",
   "authors": ["Nobody <nobody@example.invalid>"], "download_count": 1234},
  {"id": "another", "name": "Another", "description": "Listed, never installed",
   "authors": [], "download_count": 7}
]}
JSON
  tar -czf "$registry/extensions/sample/download" -C "$scenarios_dir/registry" themes
  python3 -m http.server "$registry_port" --bind 127.0.0.1 --directory "$registry" \
    >"$work/registry.log" 2>&1 &
  registry_pid=$!
  for _ in $(seq 50); do
    (exec 3<>"/dev/tcp/127.0.0.1/$registry_port") 2>/dev/null && break
    sleep 0.1
  done
fi

header() { sed -n "s/^# *$2: *//p" "$1" | head -n1; }

failed=()
for file in "${files[@]}"; do
  name=$(basename -- "$file" .script)
  [[ -f $file ]] || { echo "no such scenario: $name" >&2; exit 2; }
  mode=$(header "$file" mode)
  size=$(header "$file" size)
  # The steps: every line that is not blank or a comment, joined by `;`.
  script=$(grep -v -E '^\s*(#|$)' "$file" | paste -sd ';')

  home=$work/$name
  mkdir -p -- "$home"
  env=(
    XDG_CONFIG_HOME="$home/config" XDG_DATA_HOME="$home/data"
    MONEY_MANAGER_TODAY="$today"
    MONEY_MANAGER_SIZE="${size:-1240x800}" MONEY_MANAGER_SCRIPT="$script"
    MONEY_MANAGER_THEMES_API="http://127.0.0.1:$registry_port"
  )
  case $mode in
    demo) env+=(MONEY_MANAGER_DEMO=1) ;;
    fresh) ;;
    legacy)
      mkdir -p -- "$home/data/app.akergez.MoneyManager/ledger" "$home/config/app.akergez.MoneyManager"
      cp -- "$scenarios_dir/legacy/staging.rdx" "$home/data/app.akergez.MoneyManager/ledger/staging.rdx"
      printf '%s\n' '{"source":42,"remote":{"endpoint":"https://old.invalid","secret_access_key":"old-test-key"},"default_currency":"RUB","theme":"light"}' \
        > "$home/config/app.akergez.MoneyManager/settings.json"
      ;;
    *) echo "$name: header must say '# mode: demo', '# mode: fresh' or '# mode: legacy'" >&2; exit 2 ;;
  esac

  $headless && swaymsg -q output HEADLESS-1 resolution "${size:-1240x800}"

  status=0
  env "${env[@]}" timeout "$timeout_s" "$app" >"$home/app.log" 2>&1 || status=$?

  if (( status == 0 )) && [[ $mode == legacy ]]; then
    [[ -f $home/data/app.akergez.MoneyManager/ledger-tresse/ledger.rdx ]] || status=1
    [[ ! -e $home/data/app.akergez.MoneyManager/ledger-tresse/.tresse/remotes.toml ]] || status=1
    python3 - "$home/config/app.akergez.MoneyManager/settings.json" \
      "$scenarios_dir/legacy/staging.rdx" "$home/data/app.akergez.MoneyManager/ledger.pre-tresse/staging.rdx" <<'PY_CHECK' || status=1
import json, sys
from pathlib import Path
assert Path(sys.argv[2]).read_bytes() == Path(sys.argv[3]).read_bytes()
settings = json.load(open(sys.argv[1]))
assert 'source' not in settings and 'remote' not in settings
assert settings['theme'] == 'light' and settings['default_currency'] == 'RUB'
PY_CHECK
    env "${env[@]}" MONEY_MANAGER_SCRIPT="wait:2000;expect:stage=workspace;expect:records=1;expect:remote=no;expect:dialog=closed;quit" \
      timeout "$timeout_s" "$app" >"$home/restart.log" 2>&1 || status=$?
  fi

  if (( status == 0 )); then
    echo "ok    $name"
  else
    case $status in
      124) why="did not finish in ${timeout_s}s" ;;
      1)   why=$(grep -m1 '^script: expected' "$home/app.log" || echo 'exit status 1') ;;
      *)   why="exit status $status" ;;
    esac
    echo "FAIL  $name — $why"
    grep -E 'panicked|ERROR' "$home/app.log" | head -n 5 | sed 's/^/        /' || true
    failed+=("$name")
  fi
done

echo
if (( ${#failed[@]} )); then
  echo "${#failed[@]} of ${#files[@]} failed: ${failed[*]}"
  exit 1
fi
echo "all ${#files[@]} passed"

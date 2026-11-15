#!/bin/sh
# Records the Freerouting replay fixture: the Eagle blinky board (synthetic
# test data) is exported as a Specctra DSN file, routed by Freerouting and
# its session (SES) kept next to it. `crates/ew-core/tests/freerouting.rs`
# replays the session and fails when the DSN export changes, so record again
# after changing the DSN writer.
#
# Usage (from the repository root):
#   FREEROUTING=/path/to/freerouting scripts/record-freerouting.sh
#
# FREEROUTING is the Freerouting launcher, for example
# `/Applications/freerouting.app/Contents/MacOS/freerouting` or a
# `java -jar freerouting.jar` wrapper. Analytics are switched off (-da).
set -eu

: "${FREEROUTING:?set FREEROUTING to the Freerouting launcher}"
root=$(pwd)
fixtures="$root/crates/ew-core/tests/fixtures/freerouting"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cargo build --release -p ew-cli
ewerk="$root/target/release/ewerk"

"$ewerk" import eagle crates/ew-eagle/tests/fixtures/blinky/blinky.sch "$work/blinky" >/dev/null
mkdir -p "$fixtures"
"$ewerk" board export-dsn "$work/blinky" -o "$fixtures/blinky.dsn"
"$FREEROUTING" -de "$fixtures/blinky.dsn" -do "$fixtures/blinky.ses" -da \
  --gui.enabled=false --user_data_path="$work/freerouting" \
  --router.autorouter.max_passes=40
echo "Recorded $fixtures/blinky.ses"

#!/bin/bash

# Build script for ProductionDeck device-specific firmware.
#
# The workspace root is not pinned to the embedded target, so firmware builds
# pass `--target thumbv6m-none-eabi` explicitly.

set -euo pipefail

TARGET=thumbv6m-none-eabi
DEVICES=(
  mini revised-mini original original-v2 xl plus mk2 neo plus-xl
  module6 module15 module32 studio studio-cora
)

build_one() {
  local device=$1
  echo "Building StreamDeck $device..."
  cargo build --release -p productiondeck-firmware --target "$TARGET" --bin "$device"
  echo "✓ target/$TARGET/release/$device"
}

if [[ $# -gt 0 ]]; then
  build_one "$1"
else
  for device in "${DEVICES[@]}"; do
    build_one "$device"
  done
fi

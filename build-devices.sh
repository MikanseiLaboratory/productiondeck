#!/bin/bash

# Build script for ProductionDeck device-specific firmware
# This script builds firmware for different StreamDeck models

echo "=== ProductionDeck Multi-Device Build Script ==="
echo

# List of devices to build
devices=("revised-mini" "original" "original-v2" "xl" "plus")

echo "Available device targets:"
for device in "${devices[@]}"; do
    echo "  - $device"
done
echo

# Check if specific device was requested
if [ "$1" != "" ]; then
    device=$1
    if [[ " ${devices[@]} " =~ " ${device} " ]]; then
        echo "Building firmware for StreamDeck $device..."
        cargo build --release --bin $device
        echo "✓ Build complete: target/thumbv6m-none-eabi/release/$device"
        echo "To upload to device: cargo run --release --bin $device"
    else
        echo "Error: Unknown device '$device'"
        echo "Available devices: ${devices[*]}"
        exit 1
    fi
else
    echo "Building firmware for all devices..."
    echo

    for device in "${devices[@]}"; do
        echo "Building StreamDeck $device..."
        if cargo build --release --bin $device; then
            echo "✓ $device build successful"
        else
            echo "✗ $device build failed"
        fi
        echo
    done

    echo "All builds completed!"
    echo
    echo "Usage examples:"
    echo "  cargo run --release --bin revised-mini  # Run Mini 2022 firmware"
    echo "  cargo run --release --bin xl            # Run XL firmware"
    echo "  ./build-devices.sh revised-mini         # Build only Mini 2022 firmware"
fi
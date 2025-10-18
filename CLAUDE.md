# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenDeck plugin for Ajazz AKP05 and Mirabox N4 stream deck devices. It's written in Rust and forked from the opendeck-akp03 plugin. The plugin enables OpenDeck to communicate with these hardware devices, handling button presses, encoder rotations, touchscreen interactions, and image rendering on LCD buttons.

**Current Status**: The plugin structure is complete but USB VID/PID values and input mappings are placeholders pending actual hardware availability for testing.

## Build Commands

```sh
# Build for all platforms (requires Docker, mingw-w64-gcc, and just)
just package

# Build for specific platforms
just build-linux    # x86_64-unknown-linux-gnu
just build-win      # x86_64-pc-windows-gnu
just build-mac      # universal2-apple-darwin (requires Docker)

# Development build (native platform)
cargo build --release

# Clean all build artifacts
just clean
```

## Release Process

```sh
# Bump version (updates manifest.json and Cargo.toml)
just bump

# Create git tag with changelog
just tag

# Full release workflow
just release  # Runs: bump -> package -> tag
```

## Architecture

### Core Components

**Device Communication Layer** (`src/device.rs`)
- `device_task()`: Main task that initializes devices, registers them with OpenDeck, and manages lifecycle
- `device_events_task()`: Reads input events from hardware and forwards to OpenDeck
- `handle_set_image()`: Receives image data from OpenDeck (as data URLs) and renders to LCD buttons
- `handle_error()`: Centralizes error handling and device cleanup/deregistration

**Device Discovery** (`src/watcher.rs`)
- `watcher_task()`: Scans for devices on startup and watches for USB connect/disconnect events
- Uses `mirajazz::DeviceWatcher` to detect device lifecycle events
- Spawns/cancels `device_task()` instances dynamically

**Input Processing** (`src/inputs.rs`)
- `process_input()`: Maps raw USB HID input codes to OpenDeck events
- Handles three input types: button presses, encoder rotations, encoder presses
- **IMPORTANT**: Contains placeholder mappings that need verification with real hardware

**Device Configuration** (`src/mappings.rs`)
- Device constants: 2x5 button grid (10 buttons), 4 encoders, 4 touchscreen zones
- VID/PID definitions (currently placeholders: 0xXXXX, 0xYYYY)
- `Kind` enum for device variants (Akp05, N4)
- Image format configuration (60x60 JPEG, 90° rotation)

**Plugin Entry Point** (`src/main.rs`)
- Implements OpenDeck plugin protocol via `openaction` crate
- Global event handlers for `plugin_ready`, `set_image`, `set_brightness`
- Manages task lifecycle with `TaskTracker` and `CancellationToken`
- Signal handling for graceful shutdown (SIGTERM on Unix, pending fix for Windows)

### Key Dependencies

- `mirajazz` (0.9.0): HID communication library for stream deck-like devices
- `openaction` (1.1.5): OpenDeck plugin SDK
- `tokio`: Async runtime with full features
- `image`: Image processing (BMP, JPEG)

### Global State

Three static `LazyLock` globals coordinate device management:
- `DEVICES`: `RwLock<HashMap<String, Device>>` - Active device instances
- `TOKENS`: `RwLock<HashMap<String, CancellationToken>>` - Task cancellation tokens
- `TRACKER`: `Mutex<TaskTracker>` - Tracks all async tasks

Device IDs use format: `{DEVICE_NAMESPACE}-{serial}` where namespace is "n4"

### Touchscreen Architecture

The AKP05/N4 touchscreen follows the Stream Deck+ model:
- 110x14mm LCD strip divided into 4 touch zones
- Each zone is associated with one encoder
- OpenDeck handles all touchscreen functionality automatically (rendering, swipe gestures, tap events)
- Plugin only registers the device with `touch_zones: 4` parameter
- Touch zones are NOT separate buttons; they belong to encoders

## Critical TODOs

**Hardware-Dependent Values** (search codebase for `TODO`):

1. **USB Identifiers** (`src/mappings.rs:29-33`):
   - Replace `AJAZZ_VID`, `MIRABOX_VID`, `AKP05_PID`, `N4_PID` with actual values
   - Update usage page/usage ID in device queries if needed

2. **Input Mappings** (`src/inputs.rs`):
   - Line 18: Button input codes (0..=10 range)
   - Line 29-33: Touchscreen tap codes (0x40..=0x43 placeholders)
   - Line 37: Encoder rotation codes (0x90/0x91, 0x50/0x51, 0x60/0x61, 0x70/0x71)
   - Line 41: Encoder press codes (0x33, 0x35, 0x34, 0x36)

3. **Protocol Version** (`src/mappings.rs:75-79`):
   - Verify devices use protocol version 3

4. **Image Format** (`src/mappings.rs:82-98`):
   - Confirm 60x60 size, 90° rotation, JPEG format

## Device Differences from AKP03

- **Buttons**: 10 buttons (2x5 grid) vs. AKP03's 9 buttons
- **Encoders**: 4 encoders vs. AKP03's 3 encoders
- **Touchscreen**: Similar to Stream Deck+ architecture (not present in AKP03)

## Development Notes

- Linux is the primary development platform
- Windows/Mac have minimal support but cross-compilation is configured
- Log level is set to `Info` in production; use `Debug` for troubleshooting input mappings
- Device namespace "n4" is shared between Ajazz AKP05 and Mirabox N4 variants
- The plugin compiles to platform-specific binaries referenced in `manifest.json`

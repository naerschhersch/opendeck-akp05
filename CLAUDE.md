# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenDeck plugin for Ajazz AKP05 and Mirabox N4 stream deck devices. It's written in Rust and forked from [opendeck-akp03](https://github.com/4ndv/opendeck-akp03). The plugin enables OpenDeck to communicate with these hardware devices, handling button presses, encoder rotations, touchscreen interactions, and image rendering on LCD buttons.

**Requires OpenDeck 2.5.0 or newer**

### Supported Devices

- **Mirabox N4** - VID: 0x6603, PID: 0x1007 (confirmed with hardware)
- **Ajazz AKP05** - USB ID not yet known (hardware not available for testing)

**Current Status**: The plugin structure is complete. Mirabox N4 hardware has been tested. Ajazz AKP05 USB VID/PID values and input mappings are placeholders pending actual hardware availability.

### Device Layout

Both devices have similar layouts to Elgato Stream Deck+:
- **2x5 grid** of LCD buttons (10 total) - more than Stream Deck+'s 2x4 (8 buttons)
- **4 rotary encoders** with push function
- **LCD touchscreen strip** (110x14mm, divided into 4 touch zones, one per encoder)

The devices are registered with OpenDeck as `StreamDeckPlus` type (device type 7), which enables automatic touchscreen handling for rendering, swipe gestures, and tap events.

## Build Commands

```sh
# First-time setup: Build Docker image for macOS crosscompilation
just prepare

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
- VID/PID definitions: Mirabox N4 confirmed (0x6603, 0x1007), Ajazz AKP05 placeholders (0x0300, 0x3004)
- `Kind` enum for device variants (Akp05, N4)
- `DeviceType` enum for OpenDeck registration (StreamDeck=0, StreamDeckPlus=7)
- Image format configuration (112x112 JPEG for buttons, 200x100 for touch zones, 180° rotation)

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

1. **USB Identifiers** (`src/mappings.rs`):
   - Ajazz AKP05: Replace placeholder VID (0x0300) and PID (0x3004) with actual values when hardware is available
   - Mirabox N4: VID (0x6603) and PID (0x1007) confirmed with hardware
   - Verify usage page (65440) and usage ID (1) are correct for both devices

2. **Input Mappings** (`src/inputs.rs`):
   - Button input codes (currently 0..=10 range) - verify with hardware
   - Touchscreen tap codes (0x40..=0x43) - verify with hardware
   - Encoder rotation codes: E1(0xA0/0xA1), E2(0x50/0x51), E3(0x90/0x91), E4(0x70/0x71)
   - Encoder press codes: E1(0x37), E2(0x35), E3(0x33), E4(0x36)

3. **Protocol Version** (`src/mappings.rs`):
   - Both devices set to protocol version 3 - verify with hardware testing

4. **Image Format** (`src/mappings.rs`):
   - Buttons: 112x112 JPEG, 180° rotation
   - Touch zones: 200x100 JPEG, 180° rotation
   - Verify these settings with actual hardware

## Device Differences from AKP03

- **Buttons**: 10 buttons (2x5 grid) vs. AKP03's 9 buttons
- **Encoders**: 4 encoders vs. AKP03's 3 encoders
- **Touchscreen**: Similar to Stream Deck+ architecture (not present in AKP03)

## Development Notes

- **Linux** is the primary development platform (cross-compilation configured for Windows/macOS)
- **Windows** is the primary target platform for OpenDeck users
- Log level currently set to `Debug` in `src/main.rs:125` - consider changing to `Info` for release
- Device namespace "n4" is shared between Ajazz AKP05 and Mirabox N4 variants
- The plugin compiles to platform-specific binaries referenced in `manifest.json`
- Windows signal handling needs improvement (see TODO at `src/main.rs:116`)

### Prerequisites

- Rust 1.87+ with targets: `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-gnu`
- [just](https://just.systems) command runner
- mingw-w64-gcc for Windows cross-compilation
- Docker (for macOS builds only)

### Debugging Input Mappings

When testing with real hardware to determine input codes:

1. Log level is set to `Debug` in `src/main.rs:125` which outputs all raw input events
2. Unknown inputs are logged with `EVENT Unknown code=0x{:02X} state={}`
3. Known inputs are logged as `EVENT Button/EncoderTwist/EncoderPress/TouchTap/TouchSwipe`
4. Update the pattern matching in `src/inputs.rs::process_input()` based on observed codes
5. Verify mappings match the physical layout (button grid is row-major order)

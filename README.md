![Plugin Icon](assets/icon.png)

# OpenDeck Ajazz AKP05 Plugin

An unofficial plugin for Ajazz AKP05

## OpenDeck version

Requires OpenDeck 2.5.0 or newer

## Supported devices

- Ajazz AKP05 (USB-ID noch nicht bekannt - TODO)
- Mirabox N4 (USB-ID noch nicht bekannt - TODO)

## Device Layout

Similar to Elgato Stream Deck+, but with more buttons:
- **2x5 grid** of LCD buttons (10 total) - Stream Deck+ has 2x4 (8 buttons)
- **4 rotary encoders** with push function
- **LCD touchscreen strip** (110x14mm, divided into 4 touch zones)

### Touchscreen Implementation

The touchscreen strip follows the **Stream Deck+ architecture**:
- Each of the 4 encoders has an associated touch zone on the screen
- OpenDeck handles all touchscreen functionality automatically:
  - **Rendering**: Displays action information for each encoder
  - **Swipe gestures**: Swipe left/right to switch between pages
  - **Tap events**: Tap a zone to trigger the associated action
- The plugin only needs to register the device with the correct number of touch zones
- Functionally identical to Stream Deck+ touchscreen behavior

## Platform support

- Windows: Primary platform, tested and supported
- Linux: Minimal support, may work but not regularly tested
- Mac: Minimal support, may work but not regularly tested

## Installation

1. Download an archive from releases
2. In OpenDeck: Plugins -> Install from file
3. Download [udev rules](./40-opendeck-akp05.rules) and install them by copying into `/etc/udev/rules.d/` and running `sudo udevadm control --reload-rules`
4. Unplug and plug again the device, restart OpenDeck

## Adding new devices

Read [this wiki page](https://github.com/naerschhersch/opendeck-akp05/wiki/Adding-support-for-new-devices) for more information.

## Building

### Prerequisites

You'll need:

- A Linux OS of some sort
- Rust 1.87 and up with `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` targets installed
- gcc with Windows support
- Docker
- [just](https://just.systems)

On Arch Linux:

```sh
sudo pacman -S just mingw-w64-gcc mingw-w64-binutils
```

Adding rust targets:

```sh
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-gnu
```

### Preparing environment

```sh
$ just prepare
```

This will build docker image for macOS crosscompilation

### Building a release package

```sh
$ just package
```

## Acknowledgments

This plugin is forked from [opendeck-akp03](https://github.com/4ndv/opendeck-akp03) by Andrey Viktorov.

The original plugin is heavily based on work by contributors of [elgato-streamdeck](https://github.com/streamduck-org/elgato-streamdeck) crate

![Plugin Icon](assets/icon.png)

# OpenDeck Ajazz AKP05 Plugin

An unofficial plugin for Ajazz AKP05

## OpenDeck version

Requires OpenDeck 2.5.0 or newer

## Supported devices

- Ajazz AKP05 (USB-ID noch nicht bekannt - TODO)
- Mirabox N5 (USB-ID noch nicht bekannt - TODO)

## Device Layout

Similar to Elgato Stream Deck+, but with more buttons:
- **2x5 grid** of LCD buttons (10 total) - Stream Deck+ has 2x4 (8 buttons)
- **4 rotary encoders** with push function
- **LCD touchscreen strip** (110x14mm, divided into 4 touch zones)

### Touchscreen Implementation

The 4 touchscreen zones are handled as **virtual buttons** by the mirajazz library (button indices 11-14). This means:
- OpenDeck treats them as regular buttons, allowing full customization
- Each zone can display content and respond to touch input
- Functionally identical to Stream Deck+ touchscreen behavior

## Platform support

- Linux: Guaranteed, if stuff breaks - I'll probably catch it before public release
- Mac: Best effort, no tests before release, things may break, but I probably have means to fix them
- Windows: Zero effort, no tests before release, if stuff breaks - too bad, it's up to you to contribute fixes

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

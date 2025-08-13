# QMKonnect

Detects window changes and tells your QMK keyboard what app you're using so it can switch layers automatically.

*📖 [Complete documentation](docs/llms_full.txt) - All guides in one comprehensive file*

## Overview

QMKonnect watches which window is active and sends that info to your QMK keyboard. Your keyboard can then switch layers or run commands based on what app you're using.

This tool is part of a broader ecosystem:
- **[qmk-notifier](https://github.com/dabstractor/qmk-notifier)**: QMK module that receives commands and handles layer/feature toggling on your keyboard
- **[qmk_notifier](https://github.com/dabstractor/qmk_notifier)**: Desktop application that sends commands to your keyboard via Raw HID
- **QMKonnect** (this tool): Application that detects window changes across platforms

## Features

- **Cross-Platform Support**:
  - Windows
  - macOS
  - Linux: Arch/Hyprland only

- **Core Functionality**:
  - Detects window changes in real-time
  - Sends app name and window title to your QMK keyboard
  - Low resource usage
  - Debug logging when you need it

- **Configuration**:
  - Easy to configure
  - Reloads settings automatically

## Installation

### Windows

1. Download the MSI installer: [QMKonnect.msi](https://github.com/dabstractor/qmkonnect/releases/download/v0.1.0/QMKonnect.msi)
2. Run the installer as Administrator
3. The application will start automatically and be added to Windows startup

### Arch Linux

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/linux/arch
makepkg -si
```

### Other Linux Systems
Download the release binary: [qmkonnect](https://github.com/dabstractor/qmkonnect/releases/download/v0.1.0/qmkonnect)

If you want it to start automatically, install the service file and start the service:
```
curl https://raw.githubusercontent.com/dabstractor/qmkonnect/refs/heads/main/packaging/linux/systemd/qmkonnect.service.template | sudo tee /usr/lib/systemd/user/qmkonnect.service
systemctl --user enable --now qmkonnect.service
```
If you want the service turned off when the keyboard isn't plugged in, copy the udev rules template from this project into /etc/udev/rules.d/
```
curl https://raw.githubusercontent.com/dabstractor/qmkonnect/refs/heads/main/packaging/linux/udev/99-qmkonnect.rules.template | sudo tee /etc/udev/rules.d/99-qmkonnect.rules.template
```
Create the config file, write your config to the rules, then reload udev:
```bash
qmkonnect -c
sudo qmkonnect -r
sudo udevadm control --reload && sudo udevadm trigger
```

### macOS

1. Download QMKonnect.app from the [releases page](https://github.com/dabstractor/qmkonnect/releases/download/v0.1.0/QMKonnect.dmg)
2. Copy QMKonnect.app to your Applications folder
3. Launch QMKonnect from Applications folder

### From Source

**Windows:**
```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/windows
./build-installer.ps1
```

**macOS:**
```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/macos
./build.sh
```

**Linux:**
```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect
cargo build --release
```



## QMK Firmware Setup (REQUIRED)

**IMPORTANT**: QMKonnect will not work at all without proper QMK firmware configuration. You must add the qmk-notifier module to your keyboard's firmware first.

### 1. Add the QMK Notifier Module

In your QMK keymap directory:

```bash
git submodule add https://github.com/dabstractor/qmk-notifier.git qmk-notifier
```

### 2. Enable Raw HID

In your `rules.mk`:

```make
RAW_ENABLE = yes
```

### 3. Configure Your Keymap

Add this to your `keymap.c`:

```c
#include QMK_KEYBOARD_H
#include "./qmk-notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}

// Your keymap definitions here...
```

### 4. Set Up Layer Switching

Create your layer definitions and serial commands. See the [Examples](https://dabstractor.github.io/qmkonnect/examples) for the correct implementation using `DEFINE_SERIAL_LAYERS` and `DEFINE_SERIAL_COMMANDS` macros.

### 5. Flash Your Keyboard

Build and flash your updated firmware to your keyboard. **QMKonnect cannot communicate with your keyboard until this firmware is installed.**

## Configuration

After setting up your QMK firmware, configure QMKonnect with your keyboard's Vendor ID and Product ID.

### Windows & macOS

1. Right-click the QMKonnect system tray icon
2. Select "Settings"
3. Enter your keyboard's Vendor ID (hex format, e.g., feed)
4. Enter your keyboard's Product ID (hex format, e.g., 0000)
5. Click OK to save

### Linux

Edit the configuration file at `~/.config/qmk-notifier/config.toml`.

If no file exists, create it:

```bash
qmkonnect -c
```

Set your keyboard's Vendor ID and Product ID:
```
vendor_id = 0xfeed
product_id = 0x0000
```

Then reload:

```bash
qmkonnect -r
```

Also reload your udev rules to enable hotplug detection:
```bash
sudo udevadm control --reload && sudo udevadm trigger
```

## Usage

### Windows

The application starts automatically with Windows and runs in the background with a system tray icon.

- **Start manually**: Run "QMKonnect" from Start Menu
- **Exit**: Right-click the system tray icon and select "Quit"

### macOS

- **Start**: Launch QMKonnect from Applications folder
- **Exit**: Right-click the menu bar icon and select "Quit"

### Linux

The application should start automatically when your keyboard is plugged in.
If not, you can start it manually:
```bash
qmkonnect & disown
```

## Technical Requirements

### Windows Service Implementation

- **Background Operation**: Runs silently without console windows
- **Automatic Startup**: Starts with Windows via startup folder
- **System Tray Integration**: Provides user interface through system tray icon
- **Singleton Pattern**: Prevents multiple instances from running simultaneously
- **Window Monitoring**: Properly detects window focus changes
- **Installer**: Professional MSI installer with automatic upgrade handling

### Core Functionality

- **Window Detection**: Monitors active window changes across platforms
- **QMK Integration**: Sends window information to QMK keyboards
- **Configuration Management**: User-configurable settings
- **Error Handling**: Graceful handling of errors and edge cases

## Integration with QMK

This tool works in conjunction with:
- The [qmk-notifier](https://github.com/dabstractor/qmk-notifier) QMK module running on your keyboard
- The [qmk_notifier](https://github.com/dabstractor/qmk_notifier) tool which handles the Raw HID communication

When a window focus change is detected, this application formats the data as:
`{application_class}{GS}{window_title}` where `{GS}` is the Group Separator character (0x1D).

## Default Configuration

```toml
# Your QMK keyboard's vendor ID (in hex)
vendor_id = 0xfeed

# Your QMK keyboard's product ID (in hex)
product_id = 0x0000
```

## Example Use Cases

- Automatically switch to a coding layer when your IDE is focused
- Enable media controls when music or video applications are active
- Activate application-specific macros based on the current window
- Create context-aware keyboard layouts that adapt to your workflow

## Contributing

Contributions are welcome! Feel free to submit issues or pull requests on GitHub.

## License

[MIT License](LICENSE)

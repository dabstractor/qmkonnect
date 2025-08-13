---
layout: default
icon: fas fa-play
order: 4
---

# Usage Guide

Learn how to use QMKonnect across different platforms.

## Basic Operation

QMKonnect runs in the background, detecting window changes and talking to your QMK keyboard.

### Starting QMKonnect

#### Windows
- **Automatic**: Starts with Windows if installed via MSI
- **Manual**: Run "QMKonnect" from Start Menu or system tray

#### Linux
- **Systemd Service**: `systemctl --user start qmkonnect`
- **Manual**: `qmkonnect &`
- **With Logging**: `qmkonnect -v`

#### macOS
- **Application**: Launch QMKonnect.app from Applications

### Stopping QMKonnect

#### Windows
- Right-click system tray icon → "Quit"
- Task Manager → End process

#### Linux
- `systemctl --user stop qmkonnect`
- `pkill qmkonnect`

#### macOS
- Quit from application menu
- Activity Monitor → Force Quit

## Command Line Options (Linux Only)

```bash
qmkonnect [OPTIONS]

Options:
    -c, --config         Create a default configuration file
    -r, --reload         Reload configuration from file
    -v, --verbose        Enable verbose logging
    -h, --help          Show help information
    -V, --version       Show version information
```

Windows and macOS users interact with QMKonnect through the GUI only.

## Window Detection

QMKonnect monitors active window changes and extracts:

- **Application Class**: The application identifier (e.g., "firefox", "code")
- **Window Title**: The current window title (e.g., "README.md - Visual Studio Code")

### Data Format

Information is sent to your keyboard in this format:
```
{application_class}{GS}{window_title}
```

Where `{GS}` is the Group Separator character (ASCII 0x1D).

### Examples

| Application | Window Title | Sent Data |
|-------------|--------------|-----------|
| VS Code | `main.rs - qmkonnect` | `code{GS}main.rs - qmkonnect` |
| Firefox | `GitHub - Mozilla Firefox` | `firefox{GS}GitHub - Mozilla Firefox` |
| Terminal | `~/projects/qmkonnect` | `terminal{GS}~/projects/qmkonnect` |

## Platform-Specific Features

### Windows

#### System Tray Integration
- **Icon**: Shows QMKonnect status
- **Right-click menu**:
  - Show/Hide console
  - Reload configuration
  - View logs
  - Quit application

#### Startup Behavior
- Starts with Windows
- Runs in background
- Only runs one instance

#### Logging
- Windows Event Log integration
- Console output (if enabled)
- File logging (configurable)

### Linux

#### Hyprland Integration
QMKonnect integrates directly with Hyprland's event system:

```bash
# Check Hyprland socket
echo $HYPRLAND_INSTANCE_SIGNATURE

# Monitor events manually
socat -u UNIX-CONNECT:/tmp/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock -
```

> **Note**: Only Hyprland is supported on Linux. Other window managers are not supported yet. Please contribute support for your window manager!
{: .prompt-warning }

#### Systemd Integration
```bash
# Check service status
systemctl --user status qmkonnect

# View logs
journalctl --user -u qmkonnect -f

# Enable auto-start
systemctl --user enable qmkonnect
```

### macOS

#### Accessibility Permissions
QMKonnect requires accessibility permissions to monitor windows:

1. System Preferences → Security & Privacy → Privacy
2. Select "Accessibility" from the left panel
3. Click the lock to make changes
4. Add QMKonnect to the list of allowed applications

#### Application Bundle
The macOS version runs as an app bundle with:
- Menu bar integration
- Standard app lifecycle
- System notification support

## Use Cases

QMKonnect enables context-aware keyboard behavior by sending window information to your QMK keyboard. The actual layer switching and command execution is handled by your QMK firmware using the framework's macros.

Common use cases include:
- **Development Environment**: Switch to coding layers when IDEs are active
- **Gaming Setup**: Automatically enter gaming mode for games
- **Media Control**: Activate media layers for music/video applications
- **Browser Navigation**: Enable browser-specific shortcuts
- **Terminal Commands**: Context-aware terminal shortcuts

For implementation examples, see the [QMK Integration Guide](qmk-integration) and [Examples](examples).

## Monitoring and Debugging

### Verbose Mode

Run with verbose logging to see all activity:

```bash
qmkonnect -v
```

Output includes:
- Window change events
- Application detection
- Data sent to keyboard
- Connection status
- Error messages

### Debug Mode (Linux Only)

For detailed troubleshooting on Linux:

```bash
qmkonnect --debug
```

Shows:
- Raw window data
- Filtering decisions
- Communication protocol details
- Timing information

### Log Files

On Linux, logs are available through systemd:

```bash
# View logs
journalctl --user -u qmkonnect -f
```

Windows and macOS users can view logs through the system tray interface.

## Troubleshooting

### Common Issues

1. **No window detection**:
   - Check if QMKonnect is running
   - Verify platform-specific permissions
   - Test with `qmkonnect -v`

2. **Keyboard not responding**:
   - Verify keyboard configuration
   - Check QMK firmware has notifier module
   - Test connection with `qmkonnect --test-connection`

3. **High CPU usage**:
   - Increase polling interval
   - Use application filtering
   - Check for infinite loops in logs

### Getting Help

- Check the [troubleshooting guide](troubleshooting)
- Review [GitHub issues](https://github.com/dabstractor/qmkonnect/issues)
- Enable debug logging for detailed information
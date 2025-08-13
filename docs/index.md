---
layout: default
title: Home
permalink: /
---

# QMKonnect

Cross-platform window activity notifier for QMK keyboards

[Get Started]({{ site.baseurl }}/installation) | [View on GitHub](https://github.com/dabstractor/qmkonnect) | [Complete Documentation for Agents/LLMs](llms_full.txt)

---

## What is QMKonnect?

QMKonnect watches which window is active and tells your QMK keyboard about it. Your keyboard can then switch layers or run commands based on what app you're using.

### For Users
Simple setup for everyday use with your QMK keyboard.

### For AI Agents and Developers
Complete reference documentation available for automated tools at [llms_full.txt](llms_full.txt) - designed specifically for agents and LLMs that need comprehensive system understanding.

## Key Features

- **Cross-Platform Support**: Works on Windows, Linux, and macOS
- **Real-time Detection**: Detects window focus changes
- **QMK Integration**: Talks to QMK keyboards via Raw HID
- **Low Resource Usage**: Runs in the background
- **Easy Configuration**: Simple config file

### How It Works

1. **Window Monitoring**: Watches for active window changes
2. **Data Processing**: Gets the app name and window title
3. **QMK Communication**: Sends that info to your QMK keyboard
4. **Layer Switching**: Your keyboard responds by switching layers or running macros



---

## Quick Start

1. **Download** the latest release for your platform
2. **Install** using the provided installer or package
3. **Configure** your keyboard settings
4. **Start** monitoring window changes automatically

[Installation Guide →]({{ site.baseurl }}/installation)

---

## Part of the QMK Ecosystem

QMKonnect works alongside other tools in the QMK notification ecosystem:

- **[qmk-notifier](https://github.com/dabstractor/qmk-notifier)**: QMK firmware module for handling notifications
- **[qmk_notifier](https://github.com/dabstractor/qmk_notifier)**: Core library for Raw HID communication
- **QMKonnect**: This application for cross-platform window detection

---

## Complete Documentation

📖 **[llms_full.txt]({{ site.baseurl }}/llms_full.txt)** - Complete documentation in a single file for AI systems and comprehensive reference

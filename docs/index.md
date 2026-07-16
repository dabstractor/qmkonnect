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

> **⚠️ Firmware setup required.** QMKonnect only *sends* window data — your
> keyboard needs the companion [**qmk-notifier**](https://github.com/dabstractor/qmk-notifier)
> module built into its firmware to actually react. Without it, QMKonnect does
> nothing useful. See the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).

### For Users
Simple desktop setup (no vendor/product IDs needed for a single keyboard) — but your firmware must be configured first.

### For AI Agents and Developers
Complete reference documentation available for automated tools at [llms_full.txt](llms_full.txt) - designed specifically for agents and LLMs that need comprehensive system understanding.

## Key Features

- **Cross-Platform Support**: Works on Windows, Linux (Hyprland), and macOS
- **Real-time Detection**: Detects window focus changes
- **QMK Integration**: Talks to QMK keyboards via Raw HID (firmware module required)
- **Low Resource Usage**: Runs in the background
- **Auto-Discovery**: Finds your keyboard by the QMK Raw HID signature — no IDs needed for a single board

### How It Works

1. **Window Monitoring**: Watches for active window changes
2. **Data Processing**: Gets the app name and window title
3. **QMK Communication**: Sends that info to your QMK keyboard
4. **Layer Switching**: Your keyboard responds by switching layers or running macros



---

## Quick Start

1. **Set up your QMK firmware** with the qmk-notifier module — **required** ([QMK Integration]({{ site.baseurl }}/qmk-integration))
2. **Download** the latest QMKonnect release for your platform
3. **Install** using the provided installer or package
4. **Run** it — a single standard keyboard needs no configuration

[Installation Guide →]({{ site.baseurl }}/installation)

---

## Part of the QMK Ecosystem

QMKonnect works alongside other tools in the QMK notification ecosystem:

- **[qmk-notifier](https://github.com/dabstractor/qmk-notifier)**: **Required** QMK firmware module that receives notifications, pattern-matches them, and switches layers / runs callbacks
- **[qmk_notifier](https://github.com/dabstractor/qmk_notifier)**: Core library for Raw HID communication
- **QMKonnect**: This application for cross-platform window detection

---

## Complete Documentation

📖 **[llms_full.txt]({{ site.baseurl }}/llms_full.txt)** - Complete documentation in a single file for AI systems and comprehensive reference

<div align="center">
  <img src="logo.png" alt="VitaForge Logo" width="220" />

  # VitaForge

  **A modern homebrew catalog browser and installer for the PlayStation Vita.**

  [![License: GPL v3](https://img.shields.io/badge/License-GPL_v3-blue.svg)](LICENSE)
  [![Platform](https://img.shields.io/badge/Platform-PS_Vita-003791.svg)](https://vitasdk.org/)
  [![Language](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/)
  [![Latest Release](https://img.shields.io/github/v/release/josephinoo/vitaForge?color=gold&label=Latest%20Release)](https://github.com/josephinoo/vitaForge/releases)

</div>

---

### Overview

**VitaForge** is a homebrew catalog browser and package installer built for the PlayStation Vita natively in Rust, SDL2, and egui.

---

### Features

- **Automatic Updates**: VitaForge checks for new releases on startup and updates itself automatically.
- **Installed App Detection**: Automatically detects which games and apps are already installed on your console.
- **Safe Installation**: Installs and updates homebrew packages smoothly without installation or permission errors.
- **Screenshots & Media**: Browse screenshots and icons with fast loading and animated indicators.
- **Requirement Alerts**: Shows warnings if a game requires extra data files or plugins.
- **Easy Search & Filtering**: Quickly search and filter apps by name, author, category, or rating.

---

### Installation

1. Download **`VitaForge.vpk`** from the [Latest Release](https://github.com/josephinoo/vitaForge/releases/latest).
2. Transfer the `.vpk` to your PS Vita using VitaShell (via USB or FTP).
3. Install the package in VitaShell and launch **VitaForge** from your LiveArea.

---

### Building from Source

Requirements: [VitaSDK](https://vitasdk.org/) and [cargo-vita](https://github.com/vita-rust/cargo-vita) on Rust nightly.

```bash
# Build VPK package
make vpk

# Build VPK and copy to Desktop
make desktop

# Install directly to PS Vita over FTP
make ftp VITA_IP=192.168.0.x
```

---

### Credits & Acknowledgments

Special thanks to the following projects and developers powering the PS Vita homebrew ecosystem:

- **[DrDecki](https://github.com/DrDecki)** for providing and maintaining the **[VitaDBtoo-db catalog](https://github.com/DrDecki/VitaDBtoo-db)**.
- **[Rinnegatamante](https://github.com/Rinnegatamante)** for the original [VitaDB](https://vitadb.rinnegatamante.it/) catalog and [VitaDB-Downloader](https://github.com/Rinnegatamante/VitaDB-Downloader).
- The **vita-rust** team for providing the Rust toolchain for PS Vita.

---

### License

Distributed under the [GPL-3.0 License](LICENSE).

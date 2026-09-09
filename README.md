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

**VitaForge** is a homebrew catalog browser and package installer built for the PlayStation Vita natively in Rust.

**Stack:** SDL2 (input / audio / events) + egui (layout / focus / i18n) + **vitaGL** (GPU), following the same init pattern as [VitaDB-Downloader](https://github.com/Rinnegatamante/VitaDB-Downloader): `vglSetCircularPoolSize` → `vglInitExtended(0, 960, 544, 0x1800000, NONE)` → `vglSwapBuffers`. Desktop preview keeps SDL_Renderer.

**Vita3K note:** vitaGL’s boot splash thread often crashes the emulator. We stub splash symbols at link time (same outcome as SRB2Kart’s `make HAVE_SBRK=1 NO_SPLASHSCREEN=1` — see [BUILDING-MODERN-VITAGL.md](https://github.com/Esodland/SRB2Kart-PSVita-PSTV/blob/vitagl-modern/BUILDING-MODERN-VITAGL.md)). Optional SDK rebuild: `make vitagl-nosplash` after cloning vitaGL into `vendor/vitaGL`.

The egui renderer loads precompiled GXP shaders from [imgui-vita2d](assets/shaders/egui/README.md), so its UI shaders do not require `libshacccg.suprx`. `vglInitExtended` returns a resolution-fallback flag: `GL_FALSE` is the normal result for 960×544. Startup stages and returned errors are recorded in `ux0:data/vitaforge/vitaforge.log`.

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

# Install directly to PS Vita over FTP
make ftp VITA_IP=192.168.0.x
```

### Device checklist (GPU UI)

After installing a fresh VPK, verify:

- Themes pill shows a **paintbrush** icon; Emulators shows a **gamepad**; Recent shows a **clock**.
- Scrolling the catalog keeps ~2 rows visible without long freezes while covers stream in.
- Focused card scales/glows smoothly; changing Home/Games/… pills crossfades.
- Opening Detail fades in (~150ms) without black flashes.

---

### Credits & Acknowledgments

Special thanks to the following projects and developers powering the PS Vita homebrew ecosystem:

- **[DrDecki](https://github.com/DrDecki)** for providing and maintaining the **[VitaDBtoo-db catalog](https://github.com/DrDecki/VitaDBtoo-db)**.
- **[Rinnegatamante](https://github.com/Rinnegatamante)** for the original [VitaDB](https://vitadb.rinnegatamante.it/) catalog and [VitaDB-Downloader](https://github.com/Rinnegatamante/VitaDB-Downloader).
- The **vita-rust** team for providing the Rust toolchain for PS Vita.
- **[Rinnegatamante](https://github.com/Rinnegatamante)** also for [vitaGL](https://github.com/Rinnegatamante/vitaGL), used as the on-device GPU backend for egui meshes.
- **[Northfear](https://github.com/Northfear)** for SDL2 with vitaGL integration patterns used by Vita homebrew.

---

## Contributors 

Thanks to everyone who contributes to VitaForge ❤️ 

<a href="https://github.com/josephinoo/vitaForge/graphs/contributors"> <img src="https://contrib.rocks/image?repo=josephinoo/vitaForge" /> </a>

### License

Distributed under the [GPL-3.0 License](LICENSE).

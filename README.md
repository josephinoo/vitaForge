# vitaForge

Homebrew catalog browser for the PS Vita. Browses a list of games, apps,
ports, emulators, plugins, themes etc (vitadb style), running natively on the
console with Rust + SDL2 + egui.

Right now there is no network layer, the catalog is loaded from a bundled
`assets/catalog.json` dump. `src/data/source.rs::load_catalog()` is the one
place that would change if this pointed at a real source later, the rest of
the app doesn't care where the data came from.

## Screens

- **Catalog** - grid of cards (icon, name, author, category), search box,
  category filter chips.
- **Detail** - description, version/size/downloads/updated, a download
  button that currently just prints the `download_url` (no fetcher wired up
  yet).

## Build

Needs [VitaSDK](https://vitasdk.org/) (`VITASDK` env var set) and
[`cargo-vita`](https://github.com/vita-rust/cargo-vita) on Rust nightly:

```sh
rustup toolchain install nightly
cargo +nightly install cargo-vita
```

```sh
make vpk       # target/armv7-sony-vita-newlibeabihf/release/vitaforge.vpk
make desktop   # same, dropped on ~/Desktop
make ftp VITA_IP=192.168.0.x   # push over VitaShell's ftp server
```

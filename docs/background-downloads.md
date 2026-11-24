# Background downloads (shell plugin)

Goal: download and extract a Game Data ZIP while VitaForge is closed and another game runs.

## What was tried on real hardware

| Mechanism | Result |
|---|---|
| Rust thread inside VitaForge | Suspended with the app. Not an option. |
| bgapp (`sceBgAppUtilStartBgApp`, as BGFTP) | Starts (`start=0x0`) and runs, but **dies when VitaForge is closed**. BGFTP's README adds that enlarged-memory games kill it. |
| Plugin loaded into SceShell (`*main` in `config.txt`, as FTP-for-Vita / BHBB) | Chosen: the shell never exits. |

A first plugin that also registered a QuickMenuReborn row **hung boot at the PS logo** (recovered with L held at power-on and restoring `ur0:tai/config.txt`). The plugin is now minimal, waits 30 s before touching files, and the Quick Menu row is opt-in (`WITH_QM=1`).

## Layout

- `plugin/` — C, no libc. Portable core (`jobs`, `zipx`, `md5`, `worker`) behind `plat.h` / `dl.h`; Vita glue in `plat_vita.c`, `dl_vita.c` (sceHttp), `compat.c`.
- `src/install/bg_jobs.rs` — VitaForge side of the queue (enqueue / list / control / heartbeat).
- Queue dir `ux0:data/vitaforge/bg/`: `<id>.job` (VitaForge), `<id>.state` (plugin), `<id>.ctl` (`pause|resume|cancel`), `heartbeat`.

States: queued, downloading, paused, verifying, extracting, completed, failed, cancelled. Writes are tmp + rename; downloads append to `<dest>.part` and resume with `Range`; extraction writes `<file>.vfpart` and renames after the CRC matches. All entry names and the free space are checked before anything is written (ZIP Slip, ZIP64 supported).

## Build and test

```sh
make -C plugin test                                  # host: ZIP, queue state machine, md5 (ASan/UBSan)
cargo +nightly test --target aarch64-apple-darwin bg_jobs
PATH=$VITASDK/bin:$PATH plugin/build.sh              # -> target/plugin/vitaforge_bg.suprx
```

## Install (manual, until the Settings flow exists)

1. Copy `vitaforge_bg.suprx` to `ur0:tai/`.
2. **Back up `ur0:tai/config.txt`**, then add `ur0:tai/vitaforge_bg.suprx` under `*main` (after `QuickMenuReborn.suprx` if you use the Quick Menu build). Reboot.
3. If the console hangs at the PS logo: power off, hold **L** while powering on, restore the backup.

## Known limits / not done

- **Not yet proven on hardware**: the plugin boots, survives a game, and HTTPS from the shell. The cutoff is Phase 0 of the plan.
- HTTPS certificate verification is disabled in the plugin (system root store is old); the job md5 is the integrity check. Jobs without an md5 are unverified.
- Not wired into the install flow or UI yet (`bg_jobs.rs` is unused); Vita PKG installs stay on BGDL.
- Quick Menu row untested (it was the prime suspect of the boot hang).

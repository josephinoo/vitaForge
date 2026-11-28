#!/bin/sh
# Builds plugin/vitaforge_bg.suprx. Needs the vitasdk bin dir on PATH.
set -e
cd "$(dirname "$0")"
OUT=../target/plugin; mkdir -p $OUT
# gnu17: taiHEN's TAI_CONTINUE relies on unprototyped function pointers (gone in C23)
# link stubs for QuickMenuReborn, generated from the NIDs in qmr/ (MIT, from M-Essa11/FTP-for-Vita)
vita-libs-gen qmr/QuickMenuReborn.yml $OUT/qmrstub >/dev/null && make -s -C $OUT/qmrstub >/dev/null
QM_LIBS=""; QM_DEF=""
[ "$WITH_QM" = 1 ] && QM_DEF="-DWITH_QM" && QM_LIBS="-L$OUT/qmrstub -lQuickMenuReborn_stub_weak"
arm-vita-eabi-gcc $QM_DEF -std=gnu17 -Os -fno-builtin -Wl,-q -nostdlib -nostartfiles -o $OUT/plugin.elf plugin.c worker.c jobs.c zipx.c md5.c plat_vita.c dl_vita.c compat.c bgproxy.c proxy_vita.c hook_vita.c $QM_LIBS -lz -lSceLibKernel_stub_weak -lSceIofilemgr_stub_weak -lSceKernelThreadMgr_stub_weak -lSceRtc_stub_weak -lSceHttp_stub_weak -lSceSsl_stub_weak -lSceNet_stub_weak -lSceNetCtl_stub_weak -lSceSysmodule_stub_weak -lSceAppMgr_stub_weak -ltaihen_stub -lSceKernelModulemgr_stub_weak -lgcc
vita-elf-create -e exports.yml $OUT/plugin.elf $OUT/plugin.velf
vita-make-fself -c $OUT/plugin.velf $OUT/vitaforge_bg.suprx
ls -l $OUT/vitaforge_bg.suprx

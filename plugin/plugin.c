/* VitaForge background worker, loaded into SceShell (*main in ur0:tai/config.txt).
 * The shell never exits, so downloads/extraction keep running over any game.
 * Queue protocol: see jobs.h. */
#include <psp2/kernel/threadmgr.h>
#include <psp2/kernel/modulemgr.h>
#include <psp2/io/fcntl.h>
#include <psp2/io/stat.h>
#include <psp2/rtc.h>
#include "worker.h"
#include "proxy_vita.h"
#ifdef WITH_QM /* Quick Menu row: off by default, it hung the shell at boot once */
#include "qmr/qm_reborn.h"
#endif

#define DIR_ "ux0:data/vitaforge"
#define LOG  DIR_ "/plugin_log.txt"
#define SEP_ID    "vitaforge_bg_sep"
#define PLANE_ID  "vitaforge_bg_plane"
#define LABEL_ID  "vitaforge_bg_label"
#define STATUS_ID "vitaforge_bg_status"

/* no libc in a shell plugin: format "t=<unix> <what> <n>\n" by hand */
static char *put_u(char *p, unsigned long long v)
{
	char tmp[24];
	int i = 0;
	do { tmp[i++] = '0' + v % 10; v /= 10; } while (v);
	while (i) *p++ = tmp[--i];
	return p;
}

static void log_line(const char *what, unsigned n)
{
	char line[64], *p = line;
	SceRtcTick tick;
	sceRtcGetCurrentTick(&tick);
	/* tick = microseconds since 0001-01-01; 62135596800 s to the unix epoch */
	*p++ = 't'; *p++ = '=';
	p = put_u(p, tick.tick / 1000000ULL - 62135596800ULL);
	*p++ = ' ';
	while (*what) *p++ = *what++;
	*p++ = ' ';
	p = put_u(p, n);
	*p++ = '\n';
	SceUID fd = sceIoOpen(LOG, SCE_O_WRONLY | SCE_O_CREAT | SCE_O_APPEND, 0777);
	if (fd >= 0) {
		sceIoWrite(fd, line, p - line);
		sceIoClose(fd);
	}
}

#define QDIR DIR_ "/bg/"

static int worker_thread(SceSize args, void *argp)
{
	sceKernelDelayThread(30 * 1000 * 1000); /* let SceShell finish booting */
	sceIoMkdir("ux0:data", 0777);
	sceIoMkdir(DIR_, 0777);
	sceIoMkdir(DIR_ "/bg", 0777);
	log_line("START", 0);
	/* "hook" exists only while the native bar can really deliver; VitaForge checks it */
	sceIoRemove(QDIR "hook");
	if (proxy_start(QDIR) >= 0 && hook_start(QDIR)) {
		SceUID fd = sceIoOpen(QDIR "hook", SCE_O_WRONLY | SCE_O_CREAT | SCE_O_TRUNC, 0777);
		if (fd >= 0) sceIoClose(fd);
	}
	for (unsigned n = 1;; n++) {
		worker_heartbeat(QDIR);
		if (!worker_tick(QDIR)) sceKernelDelayThread(2 * 1000 * 1000); /* idle */
		if (n % 30 == 0) log_line("n=", n);
	}
	return 0;
}

#ifdef WITH_QM
/* Quick Menu row: "VitaForge BG   Active". QuickMenuReborn must load first
 * (listed earlier in config.txt); the weak stubs make this a no-op otherwise. */
static void register_row(void)
{
	QuickMenuRebornSeparator(SEP_ID, SCE_SEPARATOR_HEIGHT);
	QuickMenuRebornRegisterWidget(PLANE_ID, NULL, plane);
	QuickMenuRebornSetWidgetSize(PLANE_ID, SCE_PLANE_WIDTH, 100.0f, 0.0f, 0.0f);
	QuickMenuRebornSetWidgetColor(PLANE_ID, 1.0f, 1.0f, 1.0f, 0.0f);
	QuickMenuRebornRegisterWidget(LABEL_ID, PLANE_ID, text);
	QuickMenuRebornSetWidgetSize(LABEL_ID, 360.0f, 75.0f, 0.0f, 0.0f);
	QuickMenuRebornSetWidgetPosition(LABEL_ID, -200.0f, 0.0f, 0.0f, 0.0f);
	QuickMenuRebornSetWidgetLabel(LABEL_ID, "VitaForge BG");
	QuickMenuRebornRegisterWidget(STATUS_ID, PLANE_ID, text);
	QuickMenuRebornSetWidgetSize(STATUS_ID, 250.0f, 75.0f, 0.0f, 0.0f);
	QuickMenuRebornSetWidgetPosition(STATUS_ID, 250.0f, 0.0f, 0.0f, 0.0f);
	QuickMenuRebornSetWidgetColor(STATUS_ID, 0.10f, 0.65f, 1.0f, 1.0f);
	QuickMenuRebornSetWidgetLabel(STATUS_ID, "Active");
}

#endif

/* same entry alias FTP-for-Vita uses, so the linker finds an entry symbol */
int _start(SceSize args, const void *argp) __attribute__((weak, alias("module_start")));

int module_start(SceSize args, const void *argp)
{
#ifdef WITH_QM
	register_row();
#endif
	SceUID t = sceKernelCreateThread("vitaforge_bg", worker_thread, 0x10000100, 0x10000, 0, 0, NULL);
	if (t >= 0)
		sceKernelStartThread(t, 0, NULL);
	return SCE_KERNEL_START_SUCCESS;
}

int module_stop(SceSize args, const void *argp)
{
	hook_stop();
#ifdef WITH_QM
	QuickMenuRebornUnregisterWidget(STATUS_ID);
	QuickMenuRebornUnregisterWidget(LABEL_ID);
	QuickMenuRebornUnregisterWidget(PLANE_ID);
	QuickMenuRebornRemoveSeparator(SEP_ID);
#endif
	return SCE_KERNEL_STOP_SUCCESS;
}

/* Our native-bar "photos" are ZIPs, so SceShell's ExportFile fails at 100%. The hook
 * moves the finished file to the job's dest and reports success.
 * Offsets per SceShell module NID are the ones PSVitaHub ships (3.60-3.73). */
#include "bgproxy.h"
#include "plat.h"
#include "proxy_vita.h"
#include <psp2/kernel/modulemgr.h>
#include <taihen.h>
#include <string.h>

static tai_hook_ref_t ref;
static SceUID hook = -1;
static char qdir_[128];

static int export_file(uint32_t *data)
{
	int res = TAI_CONTINUE(int, ref, data);
	if (res >= 0 || !data || !data[0]) return res;
	uint32_t id = *(uint32_t *)data[0];
	char dir[] = "ux0:bgdl/t/00000000";
	for (int i = 0; i < 8; i++) dir[sizeof(dir) - 2 - i] = "0123456789abcdef"[(id >> (4 * i)) & 15];
	int ours = bg_claim(qdir_, dir);
	plat_log(ours ? "export claimed" : "export passthrough", res);
	return ours ? 0 : res;
}

static uint32_t offset_for(uint32_t nid)
{
	switch (nid) {
	case 0x0552F692: return 0x1163F6; /* 3.60 retail */
	case 0x6CB01295: return 0x111D5A; /* 3.60 PDEL */
	case 0xEAB89D5C: return 0x112756; /* 3.60 PTEL */
	case 0x5549BF1F: case 0x34B4D82E: case 0x12DAC0F3: case 0x0703C828: case 0x2053B5A5:
	case 0xF476E785: case 0x939FFBE9: case 0x734D476A: return 0x11644E; /* 3.65-3.73 retail */
	case 0xE6A02F2B: return 0x111DB2; /* 3.65 PDEL */
	case 0x587F9CED: return 0x1127AE; /* 3.65 PTEL */
	default: return 0;
	}
}

int hook_start(const char *qdir)
{
	strncpy(qdir_, qdir, sizeof(qdir_) - 1);
	tai_module_info_t info;
	memset(&info, 0, sizeof(info));
	info.size = sizeof(info);
	if (taiGetModuleInfo("SceShell", &info) < 0) return 0;
	plat_log("sceshell nid", info.module_nid);
	uint32_t off = offset_for(info.module_nid);
	if (!off) return 0;
	SceKernelModuleInfo mod;
	memset(&mod, 0, sizeof(mod));
	mod.size = sizeof(mod);
	if (sceKernelGetModuleInfo(info.modid, &mod) < 0 || off + 2 > mod.segments[0].memsz) return 0;
	/* never hook blind: the target must start like a Thumb function (push / push.w) */
	uint16_t op = *(const uint16_t *)((const char *)mod.segments[0].vaddr + off);
	plat_log("export prologue", op);
	if ((op & 0xFE00) != 0xB400 && op != 0xE92D) return 0;
	hook = taiHookFunctionOffset(&ref, info.modid, 0, off, 1, export_file);
	plat_log("export hook", hook);
	return hook >= 0;
}

void hook_stop(void)
{
	if (hook >= 0) taiHookRelease(hook, ref);
	hook = -1;
}

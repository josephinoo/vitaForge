/* Vita implementation of plat.h (runs inside SceShell, no libc). */
#include "plat.h"
#include <psp2/appmgr.h>
#include <psp2/io/dirent.h>
#include <psp2/io/fcntl.h>
#include <psp2/io/stat.h>
#include <psp2/kernel/threadmgr.h>
#include <psp2/rtc.h>
#include <string.h>

int64_t plat_file_size(const char *p);

int plat_open(const char *p, int m)
{
	int fl = m == PLAT_R ? SCE_O_RDONLY : m == PLAT_W ? SCE_O_WRONLY | SCE_O_CREAT | SCE_O_TRUNC : SCE_O_WRONLY | SCE_O_CREAT | SCE_O_APPEND;
	return sceIoOpen(p, fl, 0777);
}
int plat_read(int fd, void *b, uint32_t n) { return sceIoRead(fd, b, n); }
int plat_write(int fd, const void *b, uint32_t n) { return sceIoWrite(fd, b, n); }
int64_t plat_seek(int fd, int64_t o, int w) { return sceIoLseek(fd, o, w); }
void plat_close(int fd) { sceIoClose(fd); }
int plat_rename(const char *a, const char *b)
{
	if (sceIoRename(a, b) >= 0) return 0;
	/* sceIoRename refuses to replace an existing file on ux0: drop it and retry */
	if (plat_file_size(b) >= 0) {
		sceIoRemove(b);
		if (sceIoRename(a, b) >= 0) return 0;
	}
	return -1;
}
int plat_remove(const char *p) { return sceIoRemove(p) < 0 ? -1 : 0; }

int64_t plat_file_size(const char *p)
{
	SceIoStat st;
	return sceIoGetstat(p, &st) < 0 ? -1 : (int64_t)st.st_size;
}
int plat_exists(const char *p)
{
	if (plat_file_size(p) >= 0) return 1;
	SceUID d = sceIoDopen(p);
	if (d < 0) return 0;
	sceIoDclose(d);
	return 1;
}

int plat_mkdir_p(const char *path)
{
	char t[PLAT_PATH_MAX];
	size_t n = strlen(path);
	if (n >= sizeof(t)) return -1;
	memcpy(t, path, n + 1);
	for (size_t i = 1; i <= n; i++)
		if (t[i] == '/' || t[i] == 0) {
			char c = t[i];
			t[i] = 0;
			if (t[i - 1] != ':') { /* skip the "ux0:" device prefix */
				SceUID d = sceIoDopen(t);
				if (d >= 0) sceIoDclose(d);
				else if (sceIoMkdir(t, 0777) < 0) return -1;
			}
			t[i] = c;
		}
	return 0;
}

int64_t plat_free_space(const char *path)
{
	char dev[16];
	size_t i = 0;
	while (path[i] && path[i] != ':' && i < 10) i++;
	if (path[i] != ':') return -1;
	memcpy(dev, path, i + 1);
	dev[i + 1] = 0;
	uint64_t max = 0, free_b = 0;
	return sceAppMgrGetDevInfo(dev, &max, &free_b) < 0 ? -1 : (int64_t)free_b;
}

uint64_t plat_now(void)
{
	SceRtcTick t;
	sceRtcGetCurrentTick(&t);
	return t.tick / 1000000ULL - 62135596800ULL; /* 0001-01-01 -> unix epoch */
}
void plat_sleep_ms(uint32_t ms) { sceKernelDelayThread(ms * 1000); }

int plat_list(const char *dir, void (*cb)(const char *, void *), void *u)
{
	SceUID d = sceIoDopen(dir);
	if (d < 0) return -1;
	SceIoDirent e;
	memset(&e, 0, sizeof(e));
	while (sceIoDread(d, &e) > 0) {
		if (e.d_name[0] && strcmp(e.d_name, ".") && strcmp(e.d_name, "..")) cb(e.d_name, u);
		memset(&e, 0, sizeof(e));
	}
	sceIoDclose(d);
	return 0;
}

/* Tiny arena for zlib (two blocks per stream, freed together): bump allocator
 * that resets when everything is freed. No heap exists in a plugin. */
static unsigned char arena[96 * 1024] __attribute__((aligned(8)));
static size_t arena_off;
static int arena_live;

void *plat_alloc(size_t n)
{
	n = (n + 7) & ~(size_t)7;
	if (arena_off + n > sizeof(arena)) return NULL;
	void *p = arena + arena_off;
	arena_off += n;
	arena_live++;
	return p;
}
void plat_free(void *p)
{
	if (!p) return;
	if (--arena_live == 0) arena_off = 0;
}

void plat_log(const char *tag, int64_t v)
{
	static const char hx[] = "0123456789abcdef";
	char line[96], *p = line;
	uint64_t t = plat_now();
	char d[24]; int i = 24; d[--i] = 0;
	do { d[--i] = (char)('0' + t % 10); t /= 10; } while (t);
	*p++ = 't'; *p++ = '=';
	while (d[i]) *p++ = d[i++];
	*p++ = ' ';
	for (int k = 0; tag[k] && k < 40; k++) *p++ = tag[k];
	*p++ = ' '; *p++ = '0'; *p++ = 'x';
	for (int s = 28; s >= 0; s -= 4) *p++ = hx[((uint32_t)v >> s) & 15];
	*p++ = '\n';
	int fd = sceIoOpen("ux0:data/vitaforge/bg/worker.log", SCE_O_WRONLY | SCE_O_CREAT | SCE_O_APPEND, 0777);
	if (fd >= 0) { sceIoWrite(fd, line, (uint32_t)(p - line)); sceIoClose(fd); }
}

/* Host implementation of plat.h, used by the tests. */
#include "plat.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <dirent.h>
#include <time.h>
#include <unistd.h>

int plat_open(const char *p, int m)
{
	int fl = m == PLAT_R ? O_RDONLY : m == PLAT_W ? O_WRONLY | O_CREAT | O_TRUNC : O_WRONLY | O_CREAT | O_APPEND;
	return open(p, fl, 0644);
}
int plat_read(int fd, void *b, uint32_t n) { return (int)read(fd, b, n); }
int plat_write(int fd, const void *b, uint32_t n) { return (int)write(fd, b, n); }
int64_t plat_seek(int fd, int64_t o, int w) { return lseek(fd, o, w); }
void plat_close(int fd) { close(fd); }
int plat_rename(const char *a, const char *b) { return rename(a, b); }
int plat_remove(const char *p) { return unlink(p); }
int plat_exists(const char *p) { struct stat s; return stat(p, &s) == 0; }
int64_t plat_file_size(const char *p) { struct stat s; return stat(p, &s) ? -1 : (int64_t)s.st_size; }
int64_t plat_free_space(const char *p)
{
	struct statvfs v;
	return statvfs(p, &v) ? -1 : (int64_t)v.f_bavail * (int64_t)v.f_frsize;
}
int plat_mkdir_p(const char *path)
{
	char t[1024];
	size_t n = strlen(path);
	if (n >= sizeof(t)) return -1;
	memcpy(t, path, n + 1);
	for (size_t i = 1; i <= n; i++)
		if (t[i] == '/' || t[i] == 0) {
			char c = t[i];
			t[i] = 0;
			if (mkdir(t, 0755) && !plat_exists(t)) return -1;
			t[i] = c;
		}
	return 0;
}
void *plat_alloc(size_t n) { return malloc(n); }
void plat_free(void *p) { free(p); }
uint64_t plat_now(void) { return (uint64_t)time(NULL); }
void plat_sleep_ms(uint32_t ms) { if (!getenv("PLAT_FAST")) usleep(ms * 1000); } /* tests skip backoff waits */
int plat_list(const char *dir, void (*cb)(const char *, void *), void *u)
{
	DIR *d = opendir(dir);
	if (!d) return -1;
	struct dirent *e;
	while ((e = readdir(d)))
		if (strcmp(e->d_name, ".") && strcmp(e->d_name, "..")) cb(e->d_name, u);
	closedir(d);
	return 0;
}
void plat_log(const char *tag, int64_t v) { (void)tag; (void)v; }

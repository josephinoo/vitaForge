/* Platform shim: the core (jobs/zipx/md5) only talks to the OS through this,
 * so it builds on the host (plat_posix.c, for tests) and on the Vita (plat_vita.c). */
#ifndef PLAT_H
#define PLAT_H
#include <stdint.h>
#include <stddef.h>

enum { PLAT_R = 0, PLAT_W = 1, PLAT_A = 2 }; /* read / create+truncate / append */

int      plat_open(const char *path, int mode);          /* fd >= 0 or < 0 */
int      plat_read(int fd, void *buf, uint32_t n);       /* bytes read or < 0 */
int      plat_write(int fd, const void *buf, uint32_t n);
int64_t  plat_seek(int fd, int64_t off, int whence);     /* whence: 0 set, 1 cur, 2 end */
void     plat_close(int fd);
int      plat_rename(const char *from, const char *to);
int      plat_remove(const char *path);
int      plat_exists(const char *path);
int      plat_mkdir_p(const char *path);                 /* 0 ok */
int64_t  plat_file_size(const char *path);               /* < 0 if missing */
int64_t  plat_free_space(const char *path);              /* bytes, < 0 unknown */
uint64_t plat_now(void);                                 /* unix seconds */
void     plat_sleep_ms(uint32_t ms);
/* calls cb(name) for each directory entry (no "." / ".."); 0 ok */
int      plat_list(const char *dir, void (*cb)(const char *name, void *u), void *u);
/* diagnostic breadcrumb (Vita: appends to bg/worker.log; host: no-op) */
void     plat_log(const char *tag, int64_t value);
void    *plat_alloc(size_t n);
void     plat_free(void *p);
#endif

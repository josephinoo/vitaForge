/* The shell plugin links with -nostdlib: provide the few libc symbols zlib and
 * the core need, forwarded to the sceClib* functions the system already has. */
#include <stddef.h>
#include <psp2/kernel/clib.h>

void *memcpy(void *d, const void *s, size_t n) { return sceClibMemcpy(d, s, n); }
void *memmove(void *d, const void *s, size_t n) { return sceClibMemmove(d, s, n); }
void *memset(void *d, int v, size_t n) { return sceClibMemset(d, v, n); }
int memcmp(const void *a, const void *b, size_t n) { return sceClibMemcmp(a, b, n); }
size_t strlen(const char *s) { return sceClibStrnlen(s, 0x7FFFFFFF); }
int strcmp(const char *a, const char *b) { return sceClibStrcmp(a, b); }
int strncmp(const char *a, const char *b, size_t n) { return sceClibStrncmp(a, b, n); }
char *strncpy(char *d, const char *s, size_t n) { return sceClibStrncpy(d, s, n); }
char *strcpy(char *d, const char *s) { return sceClibStrncpy(d, s, strlen(s) + 1); }

/* zlib's default zcalloc/zcfree reference these; zipx always passes its own
 * allocator, so they are never called. They only satisfy the linker. */
#include "plat.h"
void *malloc(size_t n) { return plat_alloc(n); }
void free(void *p) { plat_free(p); }

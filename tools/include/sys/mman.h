#ifndef _SYS_MMAN_H_
#define _SYS_MMAN_H_

#include <stddef.h>

#define PROT_READ  0x1
#define PROT_WRITE 0x2
#define MAP_SHARED 0x1
#define MAP_FAILED ((void *)-1)

static inline void *mmap(void *addr, size_t len, int prot, int flags, int fd, long offset) {
    (void)addr; (void)len; (void)prot; (void)flags; (void)fd; (void)offset;
    return MAP_FAILED;
}

static inline int munmap(void *addr, size_t len) {
    (void)addr; (void)len;
    return 0;
}

#endif

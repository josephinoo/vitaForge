#include <stddef.h>

void *dlopen(const char *filename, int flags) {
    (void)filename;
    (void)flags;
    return NULL;
}

int dlclose(void *handle) {
    (void)handle;
    return 0;
}

void *dlsym(void *handle, const char *symbol) {
    (void)handle;
    (void)symbol;
    return NULL;
}

char *dlerror(void) {
    return "dynamic loading not supported";
}

int fchown(int fd, unsigned int owner, unsigned int group) {
    (void)fd;
    (void)owner;
    (void)group;
    return -1;
}

long readlink(const char *path, char *buf, size_t bufsiz) {
    (void)path;
    (void)buf;
    (void)bufsiz;
    return -1;
}

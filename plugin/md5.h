#ifndef MD5_H
#define MD5_H
#include <stdint.h>
#include <stddef.h>
typedef struct { uint32_t s[4]; uint64_t n; unsigned char b[64]; } md5_t;
void md5_init(md5_t *c);
void md5_update(md5_t *c, const void *d, size_t n);
void md5_hex(md5_t *c, char out[33]); /* finalizes */
#endif

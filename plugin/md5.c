/* Public-domain style MD5 (RFC 1321), no libc. */
#include "md5.h"
#include <string.h>

static const uint32_t K[64] = {
0xd76aa478,0xe8c7b756,0x242070db,0xc1bdceee,0xf57c0faf,0x4787c62a,0xa8304613,0xfd469501,
0x698098d8,0x8b44f7af,0xffff5bb1,0x895cd7be,0x6b901122,0xfd987193,0xa679438e,0x49b40821,
0xf61e2562,0xc040b340,0x265e5a51,0xe9b6c7aa,0xd62f105d,0x02441453,0xd8a1e681,0xe7d3fbc8,
0x21e1cde6,0xc33707d6,0xf4d50d87,0x455a14ed,0xa9e3e905,0xfcefa3f8,0x676f02d9,0x8d2a4c8a,
0xfffa3942,0x8771f681,0x6d9d6122,0xfde5380c,0xa4beea44,0x4bdecfa9,0xf6bb4b60,0xbebfbc70,
0x289b7ec6,0xeaa127fa,0xd4ef3085,0x04881d05,0xd9d4d039,0xe6db99e5,0x1fa27cf8,0xc4ac5665,
0xf4292244,0x432aff97,0xab9423a7,0xfc93a039,0x655b59c3,0x8f0ccc92,0xffeff47d,0x85845dd1,
0x6fa87e4f,0xfe2ce6e0,0xa3014314,0x4e0811a1,0xf7537e82,0xbd3af235,0x2ad7d2bb,0xeb86d391};
static const unsigned char R[64] = {
7,12,17,22,7,12,17,22,7,12,17,22,7,12,17,22,5,9,14,20,5,9,14,20,5,9,14,20,5,9,14,20,
4,11,16,23,4,11,16,23,4,11,16,23,4,11,16,23,6,10,15,21,6,10,15,21,6,10,15,21,6,10,15,21};

static void block(md5_t *c, const unsigned char *p)
{
	uint32_t m[16], a = c->s[0], b = c->s[1], cc = c->s[2], d = c->s[3];
	for (int i = 0; i < 16; i++) m[i] = p[i*4] | p[i*4+1] << 8 | (uint32_t)p[i*4+2] << 16 | (uint32_t)p[i*4+3] << 24;
	for (int i = 0; i < 64; i++) {
		uint32_t f; int g;
		if (i < 16) { f = (b & cc) | (~b & d); g = i; }
		else if (i < 32) { f = (d & b) | (~d & cc); g = (5*i + 1) & 15; }
		else if (i < 48) { f = b ^ cc ^ d; g = (3*i + 5) & 15; }
		else { f = cc ^ (b | ~d); g = (7*i) & 15; }
		f += a + K[i] + m[g];
		a = d; d = cc; cc = b;
		b += (f << R[i]) | (f >> (32 - R[i]));
	}
	c->s[0] += a; c->s[1] += b; c->s[2] += cc; c->s[3] += d;
}

void md5_init(md5_t *c)
{
	c->s[0] = 0x67452301; c->s[1] = 0xefcdab89; c->s[2] = 0x98badcfe; c->s[3] = 0x10325476; c->n = 0;
}

void md5_update(md5_t *c, const void *data, size_t n)
{
	const unsigned char *p = data;
	size_t fill = (size_t)(c->n & 63);
	c->n += n;
	if (fill) {
		size_t k = 64 - fill < n ? 64 - fill : n;
		memcpy(c->b + fill, p, k);
		p += k; n -= k;
		if (fill + k < 64) return;
		block(c, c->b);
	}
	for (; n >= 64; n -= 64, p += 64) block(c, p);
	if (n) memcpy(c->b, p, n);
}

void md5_hex(md5_t *c, char out[33])
{
	uint64_t bits = c->n * 8;
	unsigned char pad = 0x80, z = 0, len[8];
	md5_update(c, &pad, 1);
	while ((c->n & 63) != 56) md5_update(c, &z, 1);
	for (int i = 0; i < 8; i++) len[i] = (unsigned char)(bits >> (8 * i));
	md5_update(c, len, 8);
	static const char hx[] = "0123456789abcdef";
	for (int i = 0; i < 16; i++) {
		unsigned char v = (unsigned char)(c->s[i / 4] >> (8 * (i % 4)));
		out[i*2] = hx[v >> 4]; out[i*2+1] = hx[v & 15];
	}
	out[32] = 0;
}

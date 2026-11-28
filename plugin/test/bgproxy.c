/* usage: bgproxy <scratch dir ending in /> */
#include "../bgproxy.h"
#include "../plat.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static void put(const char *p, const char *body)
{
	int fd = plat_open(p, PLAT_W);
	assert(fd >= 0 && plat_write(fd, body, (uint32_t)strlen(body)) == (int)strlen(body));
	plat_close(fd);
}

int main(int argc, char **argv)
{
	preq_t r;
	char h[512];
	assert(!proxy_parse("GET /dl/ab12.jpg HTTP/1.1\r\n\r\n", &r) && !r.head && !r.has_range && !strcmp(r.id, "ab12"));
	assert(proxy_reply(&r, 10, h, sizeof(h)) > 0 && strstr(h, "200 OK") && strstr(h, "Content-Length: 10\r\n"));
	assert(!proxy_parse("HEAD http://127.0.0.1:19851/dl/a.jpg HTTP/1.1\r\nrAnGe: bytes=4-\r\n\r\n", &r) && r.head && r.has_range && !r.has_end);
	assert(proxy_reply(&r, 10, h, sizeof(h)) > 0 && strstr(h, "Content-Length: 6\r\n") && strstr(h, "bytes 4-9/10"));
	assert(!proxy_parse("GET /dl/a.jpg HTTP/1.1\r\nRange: bytes=5000000000-5000000009\r\n\r\n", &r));
	assert(proxy_reply(&r, 6000000000ULL, h, sizeof(h)) > 0 && strstr(h, "bytes 5000000000-5000000009/6000000000"));
	assert(proxy_reply(&r, 10, h, sizeof(h)) < 0);
	assert(proxy_parse("POST /dl/a.jpg HTTP/1.1\r\n\r\n", &r) < 0);
	assert(proxy_parse("GET /dl/../x.jpg HTTP/1.1\r\n\r\n", &r) < 0);
	assert(proxy_parse("GET /other HTTP/1.1\r\n\r\n", &r) < 0);

	assert(argc > 1);
	char q[600], dir[600], job[700], payload[700], dest[700], body[1600];
	snprintf(q, sizeof(q), "%s", argv[1]);
	snprintf(dir, sizeof(dir), "%st", q);
	assert(!plat_mkdir_p(dir));
	snprintf(job, sizeof(job), "%sa1.job", q);
	snprintf(payload, sizeof(payload), "%s/a1.jpg", dir);
	snprintf(dest, sizeof(dest), "%sa1.zip", q);
	plat_remove(dest);

	snprintf(body, sizeof(body), "url=x\ndest=%s\nsize=4\n", dest); /* not a native-bar job */
	put(job, body);
	put(payload, "data");
	assert(!bg_claim(q, dir) && plat_exists(payload));
	snprintf(body, sizeof(body), "url=x\ndest=%s\nsize=9\nbgdl=1\n", dest); /* truncated download */
	put(job, body);
	assert(!bg_claim(q, dir) && plat_exists(payload));
	snprintf(body, sizeof(body), "url=x\ndest=%s\nsize=4\nbgdl=1\n", dest);
	put(job, body);
	assert(bg_claim(q, dir) && !plat_exists(payload) && plat_file_size(dest) == 4);
	assert(!bg_claim(q, dir));
	puts("bgproxy: all passed");
	return 0;
}

#include "bgproxy.h"
#include "jobs.h"
#include "plat.h"
#include <string.h>

static const char *num(const char *s, uint64_t *v)
{
	const char *p = s;
	for (*v = 0; *p >= '0' && *p <= '9' && *v < UINT64_MAX / 10; p++) *v = *v * 10 + (uint64_t)(*p - '0');
	return p == s ? NULL : p;
}

/* ids are what bg_jobs.rs generates: no separators, so no path can be smuggled in */
static int id_char(char c) { return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || c == '_'; }

static int take_id(const char *p, char *id, int cap)
{
	int n = 0;
	while (id_char(p[n]) && n < cap - 1) { id[n] = p[n]; n++; }
	id[n] = 0;
	return n && p[n] == '.' ? 0 : -1;
}

int proxy_parse(const char *req, preq_t *r)
{
	memset(r, 0, sizeof(*r));
	const char *p = req;
	if (!strncmp(p, "HEAD ", 5)) { r->head = 1; p += 5; }
	else if (!strncmp(p, "GET ", 4)) p += 4;
	else return -1;
	/* accept both "/dl/x" and the absolute form "http://127.0.0.1:port/dl/x" */
	while (*p && *p != ' ' && *p != '\r' && strncmp(p, "/dl/", 4)) p++;
	if (strncmp(p, "/dl/", 4) || take_id(p + 4, r->id, sizeof(r->id))) return -1;
	for (; *p; p++) {
		if (*p != '\n') continue;
		const char *k = "range:", *h = p + 1;
		while (*k && (*h | 0x20) == *k) { h++; k++; }
		if (*k) continue;
		while (*h == ' ') h++;
		if (strncmp(h, "bytes=", 6) || !(h = num(h + 6, &r->start)) || *h != '-') return -1;
		r->has_range = 1;
		r->has_end = num(h + 1, &r->end) != NULL;
		break;
	}
	return 0;
}

static int add(char *out, int cap, int n, const char *s)
{
	int l = (int)strlen(s);
	if (n < 0 || n + l >= cap) return -1;
	memcpy(out + n, s, (size_t)l + 1);
	return n + l;
}

static int add_u(char *out, int cap, int n, uint64_t v)
{
	char t[24]; int i = 24; t[--i] = 0;
	do { t[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	return add(out, cap, n, t + i);
}

int proxy_reply(const preq_t *r, uint64_t size, char *out, int cap)
{
	uint64_t start = r->has_range ? r->start : 0;
	uint64_t end = r->has_range && r->has_end && r->end < size ? r->end : size - 1;
	if (!size || start > end) return -1;
	int n = add(out, cap, 0, r->has_range ? "HTTP/1.1 206 Partial Content\r\n" : "HTTP/1.1 200 OK\r\n");
	n = add(out, cap, n, "Content-Type: image/jpeg\r\nAccept-Ranges: bytes\r\nConnection: close\r\nContent-Length: ");
	n = add_u(out, cap, n, end - start + 1);
	if (r->has_range) {
		n = add(out, cap, n, "\r\nContent-Range: bytes ");
		n = add_u(out, cap, n, start);
		n = add(out, cap, n, "-");
		n = add_u(out, cap, n, end);
		n = add(out, cap, n, "/");
		n = add_u(out, cap, n, size);
	}
	return add(out, cap, n, "\r\n\r\n");
}

static int cat3(char *d, size_t cap, const char *a, const char *b, const char *c)
{
	size_t x = strlen(a), y = strlen(b), z = strlen(c);
	if (x + y + z >= cap) return -1;
	memcpy(d, a, x); memcpy(d + x, b, y); memcpy(d + x + y, c, z + 1);
	return 0;
}

int bg_peek(const char *qdir, const char *id, char *buf, int cap,
            char *url, int url_cap, char *dest, int dest_cap, uint64_t *size)
{
	char path[600], t[32];
	if (cat3(path, sizeof(path), qdir, id, ".job")) return -1;
	int fd = plat_open(path, PLAT_R);
	if (fd < 0) return -1;
	int n = plat_read(fd, buf, (uint32_t)cap - 1);
	plat_close(fd);
	if (n <= 0) return -1;
	buf[n] = 0;
	if (!kv_get(buf, "bgdl", t, sizeof(t)) || t[0] != '1') return -1;
	if (!kv_get(buf, "url", url, url_cap) || !kv_get(buf, "dest", dest, dest_cap)) return -1;
	*size = 0;
	if (kv_get(buf, "size", t, sizeof(t))) num(t, size);
	return 0;
}

typedef struct { char id[64], name[80]; int found; } claim_t;

static void claim_cb(const char *name, void *u)
{
	claim_t *c = u;
	size_t n = strlen(name);
	/* only our disguise extensions: never d0.pdb and friends */
	if (c->found || n < 5 || n >= sizeof(c->name) || strcmp(name + n - 4, ".jpg")) return;
	if (take_id(name, c->id, sizeof(c->id))) return;
	memcpy(c->name, name, n + 1);
	c->found = 1;
}

int bg_claim(const char *qdir, const char *dir)
{
	static char buf[4096], url[1024], dest[512];
	claim_t c = { { 0 }, { 0 }, 0 };
	char from[600];
	uint64_t size;
	if (plat_list(dir, claim_cb, &c) || !c.found) return 0;
	if (bg_peek(qdir, c.id, buf, sizeof(buf), url, sizeof(url), dest, sizeof(dest), &size)) return 0;
	if (cat3(from, sizeof(from), dir, "/", c.name)) return 0;
	/* a short file is a broken download: leave the shell's error in place */
	if (size && (uint64_t)plat_file_size(from) != size) return 0;
	return plat_rename(from, dest) == 0;
}

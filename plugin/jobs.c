/* Job/state files: key=value lines, atomic writes. No libc beyond string.h. */
#include "jobs.h"
#include "plat.h"
#include <string.h>

static const char *NAMES[] = { "queued", "downloading", "paused", "verifying", "extracting", "completed", "failed", "cancelled" };
const char *job_status_name(int s) { return s >= 0 && s <= JS_CANCELLED ? NAMES[s] : "failed"; }

int kv_get(const char *t, const char *key, char *out, int cap)
{
	size_t kl = strlen(key);
	for (const char *p = t; *p;) {
		const char *e = p;
		while (*e && *e != '\n') e++;
		if ((size_t)(e - p) > kl && !memcmp(p, key, kl) && p[kl] == '=') {
			int n = (int)(e - p - kl - 1);
			if (n > 0 && p[kl + 1 + n - 1] == '\r') n--;
			if (n >= cap) n = cap - 1;
			memcpy(out, p + kl + 1, (size_t)n);
			out[n] = 0;
			return 1;
		}
		p = *e ? e + 1 : e;
	}
	out[0] = 0;
	return 0;
}

static uint64_t to_u64(const char *s)
{
	uint64_t v = 0;
	for (; *s >= '0' && *s <= '9'; s++) v = v * 10 + (uint64_t)(*s - '0');
	return v;
}

static int slurp(const char *path, char *buf, int cap)
{
	int fd = plat_open(path, PLAT_R);
	if (fd < 0) return -1;
	int n = plat_read(fd, buf, (uint32_t)cap - 1);
	plat_close(fd);
	if (n < 0) return -1;
	buf[n] = 0;
	return n;
}

static int put_s(char *buf, int cap, int n, const char *k, const char *v)
{
	int kl = (int)strlen(k), vl = (int)strlen(v);
	if (n + kl + vl + 2 >= cap) return -1;
	memcpy(buf + n, k, (size_t)kl); n += kl;
	buf[n++] = '=';
	memcpy(buf + n, v, (size_t)vl); n += vl;
	buf[n++] = '\n';
	return n;
}

static int put_u(char *buf, int cap, int n, const char *k, uint64_t v)
{
	char t[24]; int i = 24; t[--i] = 0;
	do { t[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	return put_s(buf, cap, n, k, t + i);
}

static int atomic_write(const char *path, const char *buf, int n)
{
	char tmp[600];
	size_t pl = strlen(path);
	if (pl + 5 >= sizeof(tmp)) return -1;
	memcpy(tmp, path, pl); memcpy(tmp + pl, ".tmp", 5);
	int fd = plat_open(tmp, PLAT_W);
	if (fd < 0) return -1;
	int w = plat_write(fd, buf, (uint32_t)n);
	plat_close(fd);
	if (w != n) { plat_remove(tmp); return -1; }
	if (plat_rename(tmp, path)) { plat_remove(tmp); return -1; }
	return 0;
}

int job_read(const char *path, job_t *j)
{
	static char buf[4096];
	char t[32];
	if (slurp(path, buf, sizeof(buf)) < 0) return -1;
	memset(j, 0, sizeof(*j));
	if (!kv_get(buf, "url", j->url, sizeof(j->url)) || !kv_get(buf, "dest", j->dest, sizeof(j->dest))) return -2;
	kv_get(buf, "extract_to", j->extract_to, sizeof(j->extract_to));
	kv_get(buf, "name", j->name, sizeof(j->name));
	kv_get(buf, "md5", j->md5, sizeof(j->md5));
	if (!kv_get(buf, "kind", j->kind, sizeof(j->kind))) strcpy(j->kind, "data");
	if (strcmp(j->kind, "data") && strcmp(j->kind, "vpk")) return -2;
	if (kv_get(buf, "size", t, sizeof(t))) j->size = to_u64(t);
	if (kv_get(buf, "overwrite", t, sizeof(t))) j->overwrite = t[0] == '1';
	if (kv_get(buf, "bgdl", t, sizeof(t))) j->bgdl = t[0] == '1';
	if (kv_get(buf, "created", t, sizeof(t))) j->created = to_u64(t);
	return 0;
}

int job_write(const char *path, const job_t *j)
{
	static char buf[4096];
	int n = 0;
	n = put_s(buf, sizeof(buf), n, "url", j->url);       if (n < 0) return -1;
	n = put_s(buf, sizeof(buf), n, "dest", j->dest);     if (n < 0) return -1;
	n = put_s(buf, sizeof(buf), n, "extract_to", j->extract_to); if (n < 0) return -1;
	n = put_s(buf, sizeof(buf), n, "name", j->name);     if (n < 0) return -1;
	n = put_s(buf, sizeof(buf), n, "md5", j->md5);       if (n < 0) return -1;
	n = put_s(buf, sizeof(buf), n, "kind", j->kind);     if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "size", j->size);     if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "overwrite", (uint64_t)j->overwrite); if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "bgdl", (uint64_t)j->bgdl); if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "created", j->created); if (n < 0) return -1;
	return atomic_write(path, buf, n);
}

int state_read(const char *path, state_t *s)
{
	static char buf[1024];
	char t[32];
	if (slurp(path, buf, sizeof(buf)) < 0) return -1;
	memset(s, 0, sizeof(*s));
	kv_get(buf, "status", t, sizeof(t));
	s->status = JS_FAILED;
	for (int i = 0; i <= JS_CANCELLED; i++) if (!strcmp(t, NAMES[i])) s->status = i;
	if (kv_get(buf, "downloaded", t, sizeof(t))) s->downloaded = to_u64(t);
	if (kv_get(buf, "total", t, sizeof(t))) s->total = to_u64(t);
	if (kv_get(buf, "extract_done", t, sizeof(t))) s->extract_done = to_u64(t);
	if (kv_get(buf, "extract_total", t, sizeof(t))) s->extract_total = to_u64(t);
	if (kv_get(buf, "updated", t, sizeof(t))) s->updated = to_u64(t);
	kv_get(buf, "error", s->error, sizeof(s->error));
	return 0;
}

int state_write(const char *path, const state_t *s)
{
	static char buf[1024];
	int n = 0;
	n = put_s(buf, sizeof(buf), n, "status", job_status_name(s->status)); if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "downloaded", s->downloaded);          if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "total", s->total);                    if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "extract_done", s->extract_done);      if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "extract_total", s->extract_total);    if (n < 0) return -1;
	n = put_u(buf, sizeof(buf), n, "updated", s->updated);                if (n < 0) return -1;
	n = put_s(buf, sizeof(buf), n, "error", s->error);                    if (n < 0) return -1;
	return atomic_write(path, buf, n);
}

/* Portable half of the native-download-bar bridge (host tested). The Vita half is
 * proxy_vita.c (loopback proxy) and hook_vita.c (SceShell ExportFile hook). */
#ifndef BGPROXY_H
#define BGPROXY_H
#include <stdint.h>

typedef struct {
	int head;                /* HEAD instead of GET */
	int has_range, has_end;
	uint64_t start, end;     /* inclusive, as in the Range header */
	char id[64];             /* from /dl/<id>.<ext> */
} preq_t;

/* 0 ok, <0 not a request we serve. `req` is the NUL-terminated header block. */
int proxy_parse(const char *req, preq_t *r);
/* Writes the response header for a `size` byte file; its length, or <0 if the range is unsatisfiable. */
int proxy_reply(const preq_t *r, uint64_t size, char *out, int cap);
/* Reads <qdir><id>.job if it is a native-bar job. `buf` is caller-owned scratch
 * (job_read's buffer belongs to the worker thread). 0 ok. */
int bg_peek(const char *qdir, const char *id, char *buf, int cap,
            char *url, int url_cap, char *dest, int dest_cap, uint64_t *size);
/* A finished native download sits in `dir` as <id>.<ext>: move it to the job's
 * dest so the worker picks it up. 1 if claimed (ours), 0 otherwise. */
int bg_claim(const char *qdir, const char *dir);
#endif

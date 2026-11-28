#ifndef JOBS_H
#define JOBS_H
#include <stdint.h>

/* Job + state files live in a queue dir, one pair per job:
 *   <id>.job    written by VitaForge (immutable once queued)
 *   <id>.state  written by the plugin
 *   <id>.ctl    "pause" | "resume" | "cancel", written by VitaForge, consumed by the plugin
 * Format: one "key=value" per line. Writes are tmp + rename (atomic). */

enum job_status { JS_QUEUED, JS_DOWNLOADING, JS_PAUSED, JS_VERIFYING, JS_EXTRACTING, JS_COMPLETED, JS_FAILED, JS_CANCELLED };

typedef struct {
	char url[1024], dest[512], extract_to[512], name[128], md5[33];
	char kind[8];            /* "data": zip saved at dest, extracted into extract_to, then deleted;
                                "vpk": file kept at dest */
	uint64_t size;           /* 0 = unknown */
	int overwrite;
	int bgdl;                /* downloaded by the native bar; the hook delivers `dest` */
	uint64_t created;
} job_t;

typedef struct {
	int status;              /* enum job_status */
	uint64_t downloaded, total, extract_done, extract_total, updated;
	char error[160];
} state_t;

const char *job_status_name(int s);
int job_read(const char *path, job_t *j);                 /* 0 ok, <0 error */
int job_write(const char *path, const job_t *j);
int state_read(const char *path, state_t *s);
int state_write(const char *path, const state_t *s);

/* Tiny helpers shared with the tests */
int kv_get(const char *text, const char *key, char *out, int cap); /* 1 found */
#endif

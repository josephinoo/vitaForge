/* Queue orchestration: job/state/ctl files, retries, md5, extraction.
 * Portable; the network (dl.h) and OS (plat.h) are behind interfaces. */
#include "worker.h"
#include "dl.h"
#include "jobs.h"
#include "md5.h"
#include "plat.h"
#include "zipx.h"
#include <string.h>

#define MAX_TRIES 5

typedef struct {
	char id[64], job_path[600], state_path[600], ctl_path[600];
	job_t job;
	state_t st;
	uint64_t last_save, last_ctl;
	int ctl; /* last control seen: 0 none, 1 pause, 2 cancel */
} ctx_t;

static ctx_t C;
static char found[64][64]; /* ids discovered in one scan (queue is small) */
static int nfound;

static void scan_cb(const char *name, void *u)
{
	(void)u;
	size_t n = strlen(name);
	if (n > 4 && n < 64 && !strcmp(name + n - 4, ".job") && nfound < 64) {
		memcpy(found[nfound], name, n - 4);
		found[nfound++][n - 4] = 0;
	}
}

static int cat(char *dst, size_t cap, const char *a, const char *b, const char *c)
{
	size_t la = strlen(a), lb = strlen(b), lc = strlen(c);
	if (la + lb + lc + 1 > cap) return -1;
	memcpy(dst, a, la); memcpy(dst + la, b, lb); memcpy(dst + la + lb, c, lc + 1);
	return 0;
}

static void save(int force)
{
	uint64_t now = plat_now();
	if (!force && now == C.last_save) return; /* at most once a second */
	C.last_save = now;
	C.st.updated = now;
	state_write(C.state_path, &C.st);
}

static void set(int status, const char *err)
{
	C.st.status = status;
	if (err) { strncpy(C.st.error, err, sizeof(C.st.error) - 1); C.st.error[sizeof(C.st.error) - 1] = 0; }
	else C.st.error[0] = 0;
	save(1);
}

/* Reads <id>.ctl. "resume" is consumed here. */
static void poll_ctl(void)
{
	char buf[16] = { 0 };
	int fd = plat_open(C.ctl_path, PLAT_R);
	C.ctl = 0;
	if (fd < 0) return;
	int n = plat_read(fd, buf, sizeof(buf) - 1);
	plat_close(fd);
	if (n <= 0) return;
	if (!strncmp(buf, "cancel", 6)) C.ctl = 2;
	else if (!strncmp(buf, "pause", 5)) C.ctl = 1;
	else if (!strncmp(buf, "resume", 6)) plat_remove(C.ctl_path);
}

static int cancel_requested(void)
{
	uint64_t now = plat_now();
	if (now != C.last_ctl) { C.last_ctl = now; poll_ctl(); }
	return C.ctl != 0;
}

static int dl_cb(uint64_t have, uint64_t total, void *u)
{
	(void)u;
	C.st.downloaded = have;
	if (total) C.st.total = total;
	save(0);
	return cancel_requested();
}

static int ex_cb(uint64_t done, uint64_t total, void *u)
{
	(void)u;
	C.st.extract_done = done;
	C.st.extract_total = total;
	save(0);
	return cancel_requested();
}

static const char *zerr(int rc)
{
	switch (rc) {
	case ZIPX_E_IO: return "extract: disk error";
	case ZIPX_E_FORMAT: return "extract: not a valid zip";
	case ZIPX_E_PATH: return "extract: unsafe path in zip";
	case ZIPX_E_SPACE: return "extract: not enough free space";
	case ZIPX_E_CRC: return "extract: corrupt data (CRC)";
	default: return "extract failed";
	}
}

static int verify_md5(const char *path)
{
	static unsigned char buf[16384];
	char hex[33];
	md5_t m;
	int fd = plat_open(path, PLAT_R);
	if (fd < 0) return -1;
	md5_init(&m);
	for (int n; (n = plat_read(fd, buf, sizeof(buf))) > 0;) {
		md5_update(&m, buf, (size_t)n);
		if (cancel_requested()) { plat_close(fd); return 2; }
	}
	plat_close(fd);
	md5_hex(&m, hex);
	for (int i = 0; i < 32; i++) /* case-insensitive compare */
		if ((hex[i] | 0x20) != (C.job.md5[i] | 0x20)) return 1;
	return 0;
}

/* Returns when the job reached a terminal state, was paused, or was cancelled. */
static void run(void)
{
	char part[600];
	if (cat(part, sizeof(part), C.job.dest, ".part", "")) { set(JS_FAILED, "dest path too long"); return; }

	/* the file may already be fully downloaded (crash between rename and state save) */
	int have_file = plat_file_size(C.job.dest) >= 0 && (C.job.size == 0 || (uint64_t)plat_file_size(C.job.dest) == C.job.size);
	if (!have_file) {
		int tries = 0, rc = DL_E_NET, http = 0;
		set(JS_DOWNLOADING, NULL);
		while (tries < MAX_TRIES) {
			if (cancel_requested()) goto interrupted;
			plat_log("dl_get start try", tries);
			rc = dl_get(C.job.url, part, dl_cb, NULL, &http);
			plat_log("dl_get rc", rc);
			if (rc == DL_OK || rc == DL_E_HTTP || rc == DL_E_CANCEL || rc == DL_E_IO) break;
			tries++;
			for (int s = 0; s < (2 << tries) && s < 60; s++) { /* 4,8,16,32,60 s, interruptible */
				plat_sleep_ms(1000);
				if (cancel_requested()) goto interrupted;
			}
		}
		if (rc == DL_E_CANCEL) goto interrupted;
		if (rc == DL_E_HTTP) { set(JS_FAILED, "server refused the download (HTTP error)"); return; }
		if (rc == DL_E_IO) { set(JS_FAILED, "couldn't write the download (disk full?)"); return; }
		if (rc != DL_OK) { set(JS_FAILED, "network error: gave up after 5 retries"); return; }
		if (plat_rename(part, C.job.dest)) { set(JS_FAILED, "couldn't finalise the download"); return; }
	}
	if (C.job.md5[0]) {
		set(JS_VERIFYING, NULL);
		int v = verify_md5(C.job.dest);
		if (v == 2) goto interrupted;
		if (v) { plat_remove(C.job.dest); set(JS_FAILED, v < 0 ? "couldn't read the download" : "md5 mismatch: download is corrupt"); return; }
	}
	if (!strcmp(C.job.kind, "data")) {
		set(JS_EXTRACTING, NULL);
		int rc = zipx_extract(C.job.dest, C.job.extract_to, C.job.overwrite, ex_cb, NULL);
		if (rc == ZIPX_E_CANCEL) goto interrupted;
		if (rc) { set(JS_FAILED, zerr(rc)); return; }
		plat_remove(C.job.dest);
	}
	set(JS_COMPLETED, NULL);
	return;

interrupted:
	if (C.ctl == 2) { plat_remove(part); set(JS_CANCELLED, NULL); plat_remove(C.ctl_path); }
	else set(JS_PAUSED, NULL); /* stays paused until a "resume" ctl arrives */
}

static int load(const char *q, const char *id)
{
	memset(&C, 0, sizeof(C));
	strncpy(C.id, id, sizeof(C.id) - 1);
	return (cat(C.job_path, sizeof(C.job_path), q, C.id, ".job") ||
	        cat(C.state_path, sizeof(C.state_path), q, C.id, ".state") ||
	        cat(C.ctl_path, sizeof(C.ctl_path), q, C.id, ".ctl")) ? -1 : 0;
}

int worker_tick(const char *qdir)
{
	nfound = 0;
	if (plat_list(qdir, scan_cb, NULL)) return 0;
	for (int i = 0; i < nfound; i++) {
		if (load(qdir, found[i])) continue;
		if (job_read(C.job_path, &C.job)) {
			if (state_read(C.state_path, &C.st)) { /* never reported: tell VitaForge */
				memset(&C.st, 0, sizeof(C.st));
				set(JS_FAILED, "invalid job file");
			}
			continue;
		}
		if (state_read(C.state_path, &C.st)) {
			memset(&C.st, 0, sizeof(C.st));
			C.st.status = JS_QUEUED;
			C.st.total = C.job.size;
			save(1);
		}
		if (C.st.status == JS_COMPLETED || C.st.status == JS_FAILED || C.st.status == JS_CANCELLED) continue;
		poll_ctl();
		if (C.ctl == 2) {
			char part[600];
			if (!cat(part, sizeof(part), C.job.dest, ".part", "")) plat_remove(part);
			set(JS_CANCELLED, NULL);
			plat_remove(C.ctl_path);
			return 1;
		}
		if (C.ctl == 1) { /* pause before/while idle: nothing runs */
			if (C.st.status != JS_PAUSED) set(JS_PAUSED, NULL);
			continue;
		}
		run();
		return 1;
	}
	return 0;
}

void worker_heartbeat(const char *qdir)
{
	char p[600], t[24];
	uint64_t v = plat_now();
	int i = 24;
	t[--i] = 0;
	do { t[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	if (cat(p, sizeof(p), qdir, "heartbeat", "")) return;
	int fd = plat_open(p, PLAT_W);
	if (fd < 0) return;
	plat_write(fd, t + i, (uint32_t)strlen(t + i));
	plat_close(fd);
}

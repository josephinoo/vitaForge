/* Test double for dl.h: "downloads" file:// URLs by copying, honouring resume.
 * Special URLs: file://FAIL_NET (retryable error), file://FAIL_404 (fatal). */
#include "dl.h"
#include "plat.h"
#include <string.h>

int dl_fail_net_times; /* tests set this: first N calls return DL_E_NET */
int dl_calls;

int dl_get(const char *url, const char *part, dl_progress cb, void *user, int *status)
{
	char buf[4096];
	dl_calls++;
	if (status) *status = 200;
	if (strncmp(url, "file://", 7)) return DL_E_HTTP;
	if (!strcmp(url + 7, "FAIL_404")) { if (status) *status = 404; return DL_E_HTTP; }
	if (!strcmp(url + 7, "FAIL_NET") || dl_fail_net_times-- > 0) return DL_E_NET;
	int64_t total = plat_file_size(url + 7);
	if (total < 0) return DL_E_HTTP;
	int64_t have = plat_file_size(part);
	if (have < 0) have = 0;
	if (have > total) { plat_remove(part); have = 0; }
	int in = plat_open(url + 7, PLAT_R), out = plat_open(part, PLAT_A);
	if (in < 0 || out < 0) return DL_E_IO;
	plat_seek(in, have, 0);
	for (int n; (n = plat_read(in, buf, sizeof(buf))) > 0;) {
		if (plat_write(out, buf, (uint32_t)n) != n) { plat_close(in); plat_close(out); return DL_E_IO; }
		have += n;
		if (cb && cb((uint64_t)have, (uint64_t)total, user)) { plat_close(in); plat_close(out); return DL_E_CANCEL; }
	}
	plat_close(in); plat_close(out);
	return DL_OK;
}

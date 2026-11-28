/* dl.h over the system sceHttp stack, usable from inside SceShell.
 * NOTE: certificate verification is disabled (the system root store is old);
 * the job's md5 is what protects the content. Documented in docs. */
#include "dl.h"
#include "plat.h"
#include <psp2/kernel/clib.h>
#include <psp2/libssl.h>
#include <psp2/net/http.h>
#include <psp2/net/net.h>
#include <psp2/net/netctl.h>
#include <psp2/sysmodule.h>
#include <string.h>

#define NET_MEM (256 * 1024)
static unsigned char net_mem[NET_MEM] __attribute__((aligned(8)));
static unsigned char rbuf[64 * 1024];
static int net_ready;

static int ssl_accept_all(unsigned int e, void *const c[], int n, void *u) { (void)e; (void)c; (void)n; (void)u; return 0; }

/* SceShell already runs net, ssl and http itself, so "already inited" (seen on
 * hardware: http 0x80431020, ssl 0x80435020, netctl 0x80412102) is success, and
 * we must never Term them. */
#define HTTP_ALREADY_INITED 0x80431020u
static int net_up(void)
{
	if (net_ready) return 0;
	plat_log("load_net", sceSysmoduleLoadModule(SCE_SYSMODULE_NET));
	plat_log("load_https", sceSysmoduleLoadModule(SCE_SYSMODULE_HTTPS));
	SceNetInitParam p = { net_mem, NET_MEM, 0 };
	plat_log("netinit", sceNetInit(&p));
	plat_log("netctl", sceNetCtlInit());
	plat_log("sslinit", sceSslInit(300 * 1024));
	int hi = sceHttpInit(40 * 1024);
	plat_log("httpinit", hi);
	if (hi < 0 && (unsigned)hi != HTTP_ALREADY_INITED) return -1;
	net_ready = 1;
	return 0;
}

static void u64s(char *out, uint64_t v)
{
	char t[24]; int i = 24; t[--i] = 0;
	do { t[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	strcpy(out, t + i);
}

int dl_init(void) { return net_up(); }

int dl_stream(const char *url, int ranged, uint64_t start, uint64_t end, unsigned char *buf, uint32_t cap,
              int (*sink)(const void *data, int n, void *user), void *user)
{
	int tmpl = -1, conn = -1, req = -1, rc = DL_E_NET, code = 0;
	tmpl = sceHttpCreateTemplate("VitaForge-BG/1", SCE_HTTP_VERSION_1_1, 1);
	if (tmpl < 0) return DL_E_NET;
	sceHttpSetAutoRedirect(tmpl, 1);
	/* same trade-off as dl_get; the job md5, when present, is what protects the content */
	sceHttpsSetSslCallback(tmpl, ssl_accept_all, NULL);
	conn = sceHttpCreateConnectionWithURL(tmpl, url, 0);
	if (conn < 0) goto done;
	req = sceHttpCreateRequestWithURL(conn, SCE_HTTP_METHOD_GET, url, 0);
	if (req < 0) goto done;
	if (ranged) {
		char v[64] = "bytes=";
		u64s(v + 6, start);
		strcpy(v + strlen(v), "-");
		u64s(v + strlen(v), end);
		sceHttpAddRequestHeader(req, "Range", v, SCE_HTTP_HEADER_OVERWRITE);
	}
	if (sceHttpSendRequest(req, NULL, 0) < 0) goto done;
	sceHttpGetStatusCode(req, &code);
	plat_log("stream status", code);
	/* a 200 to a mid-file range would splice the wrong bytes into the download */
	if (code != 206 && !(code == 200 && (!ranged || start == 0))) { rc = DL_E_HTTP; goto done; }
	uint64_t left = end - (ranged ? start : 0) + 1;
	while (left) {
		int n = sceHttpReadData(req, buf, cap);
		if (n <= 0) goto done;
		if ((uint64_t)n > left) n = (int)left;
		if (sink(buf, n, user)) { rc = DL_E_CANCEL; goto done; }
		left -= (uint64_t)n;
	}
	rc = DL_OK;
done:
	if (req >= 0) sceHttpDeleteRequest(req);
	if (conn >= 0) sceHttpDeleteConnection(conn);
	if (tmpl >= 0) sceHttpDeleteTemplate(tmpl);
	return rc;
}

int dl_get(const char *url, const char *part, dl_progress cb, void *user, int *status)
{
	int up = net_up();
	plat_log("net_up", up);
	if (up) return DL_E_NET;
	int64_t have = plat_file_size(part);
	if (have < 0) have = 0;
	int tmpl = -1, conn = -1, req = -1, out = -1, rc = DL_E_NET;

	tmpl = sceHttpCreateTemplate("VitaForge-BG/1", SCE_HTTP_VERSION_1_1, 1);
	plat_log("template", tmpl);
	if (tmpl < 0) return DL_E_NET;
	sceHttpSetAutoRedirect(tmpl, 1);
	sceHttpsSetSslCallback(tmpl, ssl_accept_all, NULL);
	conn = sceHttpCreateConnectionWithURL(tmpl, url, 1);
	plat_log("connection", conn);
	if (conn < 0) goto done;
	req = sceHttpCreateRequestWithURL(conn, SCE_HTTP_METHOD_GET, url, 0);
	plat_log("request", req);
	if (req < 0) goto done;
	if (have > 0) {
		char v[40] = "bytes=";
		u64s(v + 6, (uint64_t)have);
		strcpy(v + strlen(v), "-");
		sceHttpAddRequestHeader(req, "Range", v, SCE_HTTP_HEADER_OVERWRITE);
	}
	int sr = sceHttpSendRequest(req, NULL, 0);
	plat_log("send", sr);
	if (sr < 0) goto done;
	int code = 0;
	sceHttpGetStatusCode(req, &code);
	plat_log("status", code);
	if (status) *status = code;
	if (code == 416) { rc = DL_OK; goto done; } /* Range past EOF: we already have it all */
	if (code >= 500) goto done;                 /* retryable */
	if (code != 200 && code != 206) { rc = DL_E_HTTP; goto done; }
	if (code == 200 && have > 0) { plat_remove(part); have = 0; } /* server ignored Range */

	unsigned long long len = 0;
	uint64_t total = sceHttpGetResponseContentLength(req, &len) < 0 ? 0 : (uint64_t)have + len;
	out = plat_open(part, PLAT_A);
	if (out < 0) { rc = DL_E_IO; goto done; }
	for (;;) {
		int n = sceHttpReadData(req, rbuf, sizeof(rbuf));
		if (n < 0) goto done;
		if (n == 0) { rc = DL_OK; break; }
		if (plat_write(out, rbuf, (uint32_t)n) != n) { rc = DL_E_IO; break; }
		have += n;
		if (cb && cb((uint64_t)have, total, user)) { rc = DL_E_CANCEL; break; }
	}
done:
	if (out >= 0) plat_close(out);
	if (req >= 0) sceHttpDeleteRequest(req);
	if (conn >= 0) sceHttpDeleteConnection(conn);
	if (tmpl >= 0) sceHttpDeleteTemplate(tmpl);
	return rc;
}

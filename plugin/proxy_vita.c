/* Loopback HTTP proxy inside SceShell: SceDownload asks 127.0.0.1 for /dl/<id>.jpg,
 * we fetch the job's real (HTTPS, redirecting) URL with sceHttp and relay it. */
#include "bgproxy.h"
#include "dl.h"
#include "plat.h"
#include "proxy_vita.h"
#include <psp2/kernel/clib.h>
#include <psp2/kernel/threadmgr.h>
#include <psp2/net/net.h>
#include <string.h>

/* shortcut: 2 connections at once (one transfer + one probe), more get refused and retried by the shell */
#define SLOTS 2
typedef struct {
	volatile int busy;
	int sock;
	char req[2048], job[4096], url[1024], dest[512], head[512];
	unsigned char io[32 * 1024];
} slot_t;
static slot_t slots[SLOTS];
static char qdir_[128];

static int send_all(int s, const void *p, int n)
{
	while (n > 0) {
		int w = sceNetSend(s, p, (unsigned)n, 0);
		if (w <= 0) return -1;
		p = (const char *)p + w;
		n -= w;
	}
	return 0;
}

typedef struct { slot_t *s; int head_len; int sent_head; } relay_t;

static int relay(const void *data, int n, void *u)
{
	relay_t *r = u;
	if (!r->sent_head) { /* only promise a body once upstream has produced one */
		if (send_all(r->s->sock, r->s->head, r->head_len)) return -1;
		r->sent_head = 1;
	}
	return send_all(r->s->sock, data, n);
}

static void serve(slot_t *s)
{
	static const char nf[] = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
	int got = 0;
	while (got < (int)sizeof(s->req) - 1) {
		int n = sceNetRecv(s->sock, s->req + got, sizeof(s->req) - 1 - got, 0);
		if (n <= 0) return;
		got += n;
		s->req[got] = 0;
		if (sceClibStrstr(s->req, "\r\n\r\n")) break;
	}
	preq_t r;
	uint64_t size;
	if (proxy_parse(s->req, &r) || bg_peek(qdir_, r.id, s->job, sizeof(s->job), s->url, sizeof(s->url), s->dest, sizeof(s->dest), &size)) {
		send_all(s->sock, nf, sizeof(nf) - 1);
		return;
	}
	int len = proxy_reply(&r, size, s->head, sizeof(s->head));
	if (len < 0) { send_all(s->sock, nf, sizeof(nf) - 1); return; }
	if (r.head) { send_all(s->sock, s->head, len); return; }
	relay_t out = { s, len, 0 };
	uint64_t end = r.has_range && r.has_end && r.end < size ? r.end : size - 1;
	int rc = dl_stream(s->url, r.has_range, r.start, end, s->io, sizeof(s->io), relay, &out);
	plat_log("proxy stream rc", rc);
}

static int client_thread(SceSize args, void *argp)
{
	slot_t *s = *(slot_t **)argp;
	serve(s);
	sceNetSocketClose(s->sock);
	s->busy = 0;
	return sceKernelExitDeleteThread(0);
}

static int listen_thread(SceSize args, void *argp)
{
	int ls = sceNetSocket("vitaforge_prx", SCE_NET_AF_INET, SCE_NET_SOCK_STREAM, 0);
	SceNetSockaddrIn a;
	memset(&a, 0, sizeof(a));
	a.sin_len = sizeof(a);
	a.sin_family = SCE_NET_AF_INET;
	a.sin_port = sceNetHtons(PROXY_PORT);
	a.sin_addr.s_addr = sceNetHtonl(0x7F000001); /* loopback only */
	int b = ls < 0 ? ls : sceNetBind(ls, (SceNetSockaddr *)&a, sizeof(a));
	int l = b < 0 ? b : sceNetListen(ls, 4);
	plat_log("proxy listen", l);
	if (l < 0) { if (ls >= 0) sceNetSocketClose(ls); return sceKernelExitDeleteThread(0); }
	for (;;) {
		int c = sceNetAccept(ls, NULL, NULL);
		if (c < 0) { sceKernelDelayThread(1000 * 1000); continue; }
		slot_t *s = NULL;
		for (int i = 0; i < SLOTS; i++) if (!slots[i].busy) { s = &slots[i]; break; }
		SceUID t = s ? sceKernelCreateThread("vitaforge_prx_c", client_thread, 0x10000100, 0x8000, 0, 0, NULL) : -1;
		if (t < 0) { sceNetSocketClose(c); continue; }
		s->busy = 1;
		s->sock = c;
		if (sceKernelStartThread(t, sizeof(s), &s) < 0) { sceKernelDeleteThread(t); sceNetSocketClose(c); s->busy = 0; }
	}
}

int proxy_start(const char *qdir)
{
	strncpy(qdir_, qdir, sizeof(qdir_) - 1);
	if (dl_init()) return -1;
	SceUID t = sceKernelCreateThread("vitaforge_prx", listen_thread, 0x10000100, 0x4000, 0, 0, NULL);
	return t < 0 ? t : sceKernelStartThread(t, 0, NULL);
}

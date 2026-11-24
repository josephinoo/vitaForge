#include "../jobs.h"
#include "../md5.h"
#include "../plat.h"
#include "../zipx.h"
#include <stdio.h>
#include <string.h>

static int fails;
#define CHECK(c) do { if (!(c)) { printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c); fails++; } } while (0)

static void md5_of(const char *s, char out[33]) { md5_t c; md5_init(&c); md5_update(&c, s, strlen(s)); md5_hex(&c, out); }

int main(int argc, char **argv)
{
	char h[33];
	md5_of("", h);                CHECK(!strcmp(h, "d41d8cd98f00b204e9800998ecf8427e"));
	md5_of("abc", h);             CHECK(!strcmp(h, "900150983cd24fb0d6963f7d28e17f72"));
	md5_of("The quick brown fox jumps over the lazy dog", h);
	CHECK(!strcmp(h, "9e107d9d372bb6826bd81d3542a419d6"));
	{ /* split updates must match one-shot across the 64-byte boundary */
		char big[200]; memset(big, 'a', 199); big[199] = 0;
		md5_t c; md5_init(&c); md5_update(&c, big, 63); md5_update(&c, big + 63, 100); md5_update(&c, big + 163, 36); md5_hex(&c, h);
		char one[33]; md5_of(big, one); CHECK(!strcmp(h, one));
	}

	CHECK(zipx_name_ok("a/b.txt", 7));
	CHECK(zipx_name_ok("dir/", 4));
	CHECK(zipx_name_ok("..hidden", 8));            /* a dotfile, not a traversal */
	CHECK(!zipx_name_ok("../x", 4));
	CHECK(!zipx_name_ok("a/../x", 6));
	CHECK(!zipx_name_ok("a/..", 4));
	CHECK(!zipx_name_ok("/abs", 4));
	CHECK(!zipx_name_ok("a\\b", 3));
	CHECK(!zipx_name_ok("ux0:/x", 6));
	CHECK(!zipx_name_ok("", 0));

	if (argc > 1) { /* jobs roundtrip in the given scratch dir */
		char p[600]; job_t j, k; state_t s, t;
		memset(&j, 0, sizeof j);
		strcpy(j.url, "https://x/y.zip?a=b=c"); strcpy(j.dest, "ux0:data/game.zip"); strcpy(j.name, "Game");
		strcpy(j.kind, "data"); strcpy(j.md5, "d41d8cd98f00b204e9800998ecf8427e"); j.size = 5368709120ULL; j.overwrite = 1; j.created = 1795000000;
		snprintf(p, sizeof p, "%s/1.job", argv[1]);
		CHECK(job_write(p, &j) == 0);
		CHECK(job_read(p, &k) == 0);
		CHECK(!strcmp(k.url, j.url) && !strcmp(k.dest, j.dest) && k.size == j.size && k.overwrite == 1 && !strcmp(k.kind, "data"));
		memset(&s, 0, sizeof s); s.status = JS_EXTRACTING; s.downloaded = 7; s.total = 9; s.extract_done = 3; strcpy(s.error, "boom");
		snprintf(p, sizeof p, "%s/1.state", argv[1]);
		CHECK(state_write(p, &s) == 0);
		CHECK(state_read(p, &t) == 0 && t.status == JS_EXTRACTING && t.downloaded == 7 && t.extract_done == 3 && !strcmp(t.error, "boom"));
		snprintf(p, sizeof p, "%s/nope.job", argv[1]);
		CHECK(job_read(p, &k) < 0);
		CHECK(!plat_exists("/nonexistent/x"));
	}
	printf(fails ? "unit: %d FAILED\n" : "unit: all passed\n", fails);
	return fails != 0;
}

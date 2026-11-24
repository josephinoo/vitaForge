/* Streaming ZIP extractor: ZIP64, stored + deflate, bounded memory.
 * No libc beyond zlib and plat.h, so it also builds into the shell plugin. */
#include "zipx.h"
#include "plat.h"
#include <string.h>
#include <zlib.h>

#define CHUNK 32768
static unsigned char in_buf[CHUNK], out_buf[CHUNK];
static char name_buf[1024], path_buf[1536 + 16];

static uint32_t u16(const unsigned char *p) { return p[0] | p[1] << 8; }
static uint32_t u32(const unsigned char *p) { return p[0] | p[1] << 8 | (uint32_t)p[2] << 16 | (uint32_t)p[3] << 24; }
static uint64_t u64(const unsigned char *p) { return u32(p) | (uint64_t)u32(p + 4) << 32; }

int zipx_name_ok(const char *n, uint32_t len)
{
	if (len == 0 || len > 1000 || n[0] == '/') return 0;
	uint32_t comp = 0; /* length of the current path component */
	int dots = 1;      /* component is all dots so far */
	for (uint32_t i = 0; i <= len; i++) {
		char c = i < len ? n[i] : '/';
		if (c == 0 || c == '\\' || c == ':' || (unsigned char)c < 0x20) return 0;
		if (c == '/') {
			if (comp > 0 && dots && comp >= 2) return 0; /* ".." or "..." */
			comp = 0;
			dots = 1;
		} else {
			comp++;
			if (c != '.') dots = 0;
		}
	}
	return 1;
}

static int read_at(int fd, int64_t off, void *buf, uint32_t n)
{
	if (plat_seek(fd, off, 0) < 0) return -1;
	uint32_t got = 0;
	while (got < n) {
		int r = plat_read(fd, (char *)buf + got, n - got);
		if (r <= 0) return -1;
		got += (uint32_t)r;
	}
	return 0;
}

typedef struct { /* one central-directory entry */
	uint32_t method, crc, name_len;
	uint64_t csize, usize, lho;
	uint32_t next_off_delta; /* bytes to skip to the next entry */
} entry_t;

/* Reads the entry starting at cd_off; name goes to name_buf. */
static int read_entry(int fd, int64_t off, entry_t *e)
{
	unsigned char h[46];
	if (read_at(fd, off, h, 46) || u32(h) != 0x02014b50) return ZIPX_E_FORMAT;
	uint32_t flags = u16(h + 8), nlen = u16(h + 28), elen = u16(h + 30), clen = u16(h + 32);
	if (flags & 1) return ZIPX_E_FORMAT; /* encrypted */
	if (nlen >= sizeof(name_buf) || elen > 4096) return ZIPX_E_FORMAT;
	e->method = u16(h + 10);
	e->crc = u32(h + 16);
	e->csize = u32(h + 20);
	e->usize = u32(h + 24);
	e->lho = u32(h + 42);
	e->name_len = nlen;
	e->next_off_delta = 46 + nlen + elen + clen;
	if (read_at(fd, off + 46, name_buf, nlen)) return ZIPX_E_IO;
	name_buf[nlen] = 0;
	if (e->csize == 0xFFFFFFFFu || e->usize == 0xFFFFFFFFu || e->lho == 0xFFFFFFFFu) {
		static unsigned char ex[4096];
		if (read_at(fd, off + 46 + nlen, ex, elen)) return ZIPX_E_IO;
		for (uint32_t p = 0; p + 4 <= elen;) {
			uint32_t id = u16(ex + p), sz = u16(ex + p + 2), q = p + 4;
			if (q + sz > elen) return ZIPX_E_FORMAT;
			if (id == 1) { /* ZIP64 extended info: only the fields that were 0xFFFFFFFF */
				if (e->usize == 0xFFFFFFFFu) { if (q + 8 > p + 4 + sz) return ZIPX_E_FORMAT; e->usize = u64(ex + q); q += 8; }
				if (e->csize == 0xFFFFFFFFu) { if (q + 8 > p + 4 + sz) return ZIPX_E_FORMAT; e->csize = u64(ex + q); q += 8; }
				if (e->lho == 0xFFFFFFFFu) { if (q + 8 > p + 4 + sz) return ZIPX_E_FORMAT; e->lho = u64(ex + q); }
			}
			p += 4 + sz;
		}
	}
	return ZIPX_OK;
}

/* Finds the central directory: offset and entry count. */
static int find_cd(int fd, int64_t fsize, int64_t *cd_off, uint64_t *count)
{
	static unsigned char tail[65536 + 22 + 64];
	int64_t span = fsize < (int64_t)sizeof(tail) ? fsize : (int64_t)sizeof(tail);
	if (span < 22 || read_at(fd, fsize - span, tail, (uint32_t)span)) return ZIPX_E_FORMAT;
	for (int64_t i = span - 22; i >= 0; i--) {
		if (u32(tail + i) != 0x06054b50) continue;
		*count = u16(tail + i + 10);
		*cd_off = u32(tail + i + 16);
		if (*count == 0xFFFF || *cd_off == 0xFFFFFFFFu) { /* ZIP64 */
			unsigned char loc[20], z[56];
			int64_t eocd_abs = fsize - span + i;
			if (eocd_abs < 20 || read_at(fd, eocd_abs - 20, loc, 20) || u32(loc) != 0x07064b50) return ZIPX_E_FORMAT;
			if (read_at(fd, (int64_t)u64(loc + 8), z, 56) || u32(z) != 0x06064b50) return ZIPX_E_FORMAT;
			*count = u64(z + 32);
			*cd_off = (int64_t)u64(z + 48);
		}
		return ZIPX_OK;
	}
	return ZIPX_E_FORMAT;
}

static int join(const char *dir, const char *name, const char *suffix)
{
	size_t d = strlen(dir), n = strlen(name), s = strlen(suffix);
	if (d + 1 + n + s >= sizeof(path_buf)) return -1;
	memcpy(path_buf, dir, d);
	size_t o = d;
	if (d && dir[d - 1] != '/') path_buf[o++] = '/';
	memcpy(path_buf + o, name, n);
	memcpy(path_buf + o + n, suffix, s + 1);
	return 0;
}

static void *zalloc_cb(void *o, unsigned items, unsigned size) { (void)o; return plat_alloc((size_t)items * size); }
static void zfree_cb(void *o, void *p) { (void)o; plat_free(p); }

static int extract_one(int fd, const entry_t *e, const char *dir, int overwrite,
                       uint64_t *done, uint64_t total, zipx_progress cb, void *user)
{
	if (name_buf[e->name_len - 1] == '/') { /* directory entry */
		if (join(dir, name_buf, "")) return ZIPX_E_PATH;
		return plat_mkdir_p(path_buf) ? ZIPX_E_IO : ZIPX_OK;
	}
	if (join(dir, name_buf, "")) return ZIPX_E_PATH;
	if (!overwrite && plat_exists(path_buf)) { *done += e->usize; return ZIPX_OK; }
	/* make the parent directory */
	size_t last = strlen(path_buf);
	while (last && path_buf[last - 1] != '/') last--;
	if (last > 1) { path_buf[last - 1] = 0; int r = plat_mkdir_p(path_buf); path_buf[last - 1] = '/'; if (r) return ZIPX_E_IO; }
	char final_path[sizeof(path_buf)];
	strcpy(final_path, path_buf);
	if (join(dir, name_buf, ".vfpart")) return ZIPX_E_PATH;
	char part_path[sizeof(path_buf)];
	strcpy(part_path, path_buf);

	unsigned char lh[30];
	if (read_at(fd, (int64_t)e->lho, lh, 30) || u32(lh) != 0x04034b50) return ZIPX_E_FORMAT;
	int64_t data = (int64_t)e->lho + 30 + u16(lh + 26) + u16(lh + 28);
	if (plat_seek(fd, data, 0) < 0) return ZIPX_E_IO;

	int out = plat_open(part_path, PLAT_W);
	if (out < 0) return ZIPX_E_IO;
	int rc = ZIPX_OK;
	uint32_t crc = (uint32_t)crc32(0L, Z_NULL, 0);
	uint64_t left = e->csize, wrote = 0;
	z_stream zs;
	int zinit = 0;
	memset(&zs, 0, sizeof(zs));
	if (e->method == 8) {
		zs.zalloc = zalloc_cb;
		zs.zfree = zfree_cb;
		if (inflateInit2(&zs, -15) != Z_OK) { plat_close(out); return ZIPX_E_IO; }
		zinit = 1;
	} else if (e->method != 0) { plat_close(out); plat_remove(part_path); return ZIPX_E_FORMAT; }

	int stream_end = 0;
	while (rc == ZIPX_OK && !stream_end && (left > 0 || (e->method == 8 && zs.avail_in))) {
		uint32_t n = left > CHUNK ? CHUNK : (uint32_t)left;
		if (n) {
			int r = plat_read(fd, in_buf, n);
			if (r <= 0) { rc = ZIPX_E_FORMAT; break; }
			n = (uint32_t)r;
			left -= n;
		}
		if (e->method == 0) {
			if (plat_write(out, in_buf, n) != (int)n) { rc = ZIPX_E_IO; break; }
			crc = (uint32_t)crc32(crc, in_buf, n);
			wrote += n;
		} else {
			zs.next_in = in_buf;
			zs.avail_in = n;
			do {
				zs.next_out = out_buf;
				zs.avail_out = CHUNK;
				int z = inflate(&zs, Z_NO_FLUSH);
				if (z != Z_OK && z != Z_STREAM_END && z != Z_BUF_ERROR) { rc = ZIPX_E_FORMAT; break; }
				uint32_t have = CHUNK - zs.avail_out;
				if (have) {
					if (plat_write(out, out_buf, have) != (int)have) { rc = ZIPX_E_IO; break; }
					crc = (uint32_t)crc32(crc, out_buf, have);
					wrote += have;
				}
				if (z == Z_STREAM_END) { stream_end = 1; break; }
				if (z == Z_BUF_ERROR && zs.avail_out) break;
			} while (zs.avail_in || !zs.avail_out);
		}
		if (rc == ZIPX_OK && cb && cb(*done + wrote, total, user)) rc = ZIPX_E_CANCEL;
	}
	if (zinit) inflateEnd(&zs);
	plat_close(out);
	if (rc == ZIPX_OK && (wrote != e->usize || crc != e->crc)) rc = ZIPX_E_CRC;
	if (rc != ZIPX_OK) { plat_remove(part_path); return rc; }
	if (overwrite) plat_remove(final_path);
	if (plat_rename(part_path, final_path)) { plat_remove(part_path); return ZIPX_E_IO; }
	*done += wrote;
	return ZIPX_OK;
}

int zipx_extract(const char *zip_path, const char *dest_dir, int overwrite, zipx_progress cb, void *user)
{
	int64_t fsize = plat_file_size(zip_path);
	if (fsize < 0) return ZIPX_E_IO;
	if (fsize < 22) return ZIPX_E_FORMAT;
	int fd = plat_open(zip_path, PLAT_R);
	if (fd < 0) return ZIPX_E_IO;
	int64_t cd;
	uint64_t count;
	int rc = find_cd(fd, fsize, &cd, &count);
	uint64_t total = 0;
	entry_t e;
	/* pass 1: validate every name and total the size before writing anything */
	for (uint64_t i = 0, off = (uint64_t)cd; rc == ZIPX_OK && i < count; i++) {
		rc = read_entry(fd, (int64_t)off, &e);
		if (rc) break;
		if (!zipx_name_ok(name_buf, e.name_len)) { rc = ZIPX_E_PATH; break; }
		total += e.usize;
		off += e.next_off_delta;
	}
	if (rc == ZIPX_OK) {
		if (plat_mkdir_p(dest_dir)) rc = ZIPX_E_IO;
		else {
			int64_t free_b = plat_free_space(dest_dir);
			if (free_b >= 0 && (uint64_t)free_b < total + total / 50 + 1048576) rc = ZIPX_E_SPACE;
		}
	}
	/* pass 2: extract */
	uint64_t done = 0;
	for (uint64_t i = 0, off = (uint64_t)cd; rc == ZIPX_OK && i < count; i++) {
		rc = read_entry(fd, (int64_t)off, &e);
		if (rc) break;
		off += e.next_off_delta;
		rc = extract_one(fd, &e, dest_dir, overwrite, &done, total, cb, user);
	}
	plat_close(fd);
	return rc;
}

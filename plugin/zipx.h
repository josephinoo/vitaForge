#ifndef ZIPX_H
#define ZIPX_H
#include <stdint.h>

enum {
	ZIPX_OK = 0,
	ZIPX_E_IO = -1,       /* open/read/write failed */
	ZIPX_E_FORMAT = -2,   /* not a zip / truncated / unsupported method */
	ZIPX_E_PATH = -3,     /* ZIP Slip or otherwise unsafe entry name */
	ZIPX_E_SPACE = -4,    /* not enough free space */
	ZIPX_E_CRC = -5,      /* entry data doesn't match its CRC32 */
	ZIPX_E_CANCEL = -6    /* progress callback asked to stop */
};

/* Called after every chunk; return non-zero to cancel. */
typedef int (*zipx_progress)(uint64_t done, uint64_t total, void *user);

/* Extracts `zip_path` under `dest_dir` (created if missing).
 * Names are validated for the whole archive and free space checked BEFORE any
 * byte is written. Each file goes to "<name>.vfpart" and is renamed into place
 * when its CRC matches, so an interrupt never leaves a half-written real file.
 * Existing files are kept unless `overwrite`. Re-running after an interrupt
 * is safe. */
int zipx_extract(const char *zip_path, const char *dest_dir, int overwrite,
                 zipx_progress cb, void *user);

/* 1 if the entry name is safe to join under a destination directory. */
int zipx_name_ok(const char *name, uint32_t len);
#endif

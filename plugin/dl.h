#ifndef DL_H
#define DL_H
#include <stdint.h>

enum { DL_OK = 0, DL_E_NET = -1 /* retryable */, DL_E_HTTP = -2 /* fatal: 404, 403... */, DL_E_CANCEL = -3, DL_E_IO = -4 };

/* Progress hook: bytes now on disk and total (0 if unknown); non-zero cancels. */
typedef int (*dl_progress)(uint64_t have, uint64_t total, void *user);

/* Downloads `url` by APPENDING to `part_path` (ux0 can't grow files with seek),
 * resuming from the file's current size with a Range request. If the server
 * ignores Range and sends the whole file, the part file is truncated first.
 * Returns DL_OK when the file is complete. Implemented by dl_vita.c (sceHttp)
 * and dl_host.c (local files, for tests). */
int dl_get(const char *url, const char *part_path, dl_progress cb, void *user, int *http_status);
/* Loads the net/http stack; 0 ok. */
int dl_init(void);
/* GETs `url` (bytes start..end when `ranged`) and hands each chunk to `sink`
 * (non-zero aborts). `buf` is the caller's, so streams can run in parallel. Vita only. */
int dl_stream(const char *url, int ranged, uint64_t start, uint64_t end, unsigned char *buf, uint32_t cap,
              int (*sink)(const void *data, int n, void *user), void *user);
#endif

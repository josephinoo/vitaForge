#ifndef VITA_HW_IMG_H
#define VITA_HW_IMG_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

int vita_hw_init_jpeg(void);
void vita_hw_finish_jpeg(void);
int vita_decode_jpeg_hw(const uint8_t *jpeg_data, size_t size, uint8_t *out_rgba, size_t out_max_bytes, int *out_w, int *out_h);
int vita_decode_png_fast(const uint8_t *png_data, size_t size, uint8_t *out_rgba, size_t out_max_bytes, int *out_w, int *out_h);

#ifdef __cplusplus
}
#endif

#endif // VITA_HW_IMG_H

#include "vita_hw_img.h"

#if defined(__ARM_NEON) || defined(__ARM_NEON__) || defined(__vita__) || defined(PSP2)
#define STBI_NEON
#endif
#define STB_IMAGE_IMPLEMENTATION
#define STBI_STATIC
#define STBI_NO_STDIO
#define STBI_ONLY_PNG
#define STBI_ONLY_JPEG
#include "stb_image.h"

#if defined(__vita__) || defined(PSP2)

#include <psp2/sysmodule.h>
#include <psp2/jpeg.h>
#include <psp2/jpegarm.h>
#include <psp2/kernel/sysmem.h>
#include <stdlib.h>
#include <string.h>

static int g_jpeg_inited = 0;

int vita_hw_init_jpeg(void) {
    if (!g_jpeg_inited) {
        sceSysmoduleLoadModuleInternal(SCE_SYSMODULE_INTERNAL_JPEG_ARM);
        int res = sceJpegInitMJpeg(1);
        if (res >= 0) {
            g_jpeg_inited = 1;
        }
        return res;
    }
    return 0;
}

void vita_hw_finish_jpeg(void) {
    if (g_jpeg_inited) {
        sceJpegFinishMJpeg();
        sceSysmoduleUnloadModuleInternal(SCE_SYSMODULE_INTERNAL_JPEG_ARM);
        g_jpeg_inited = 0;
    }
}

int vita_decode_jpeg_hw(const uint8_t *jpeg_data, size_t size, uint8_t *out_rgba, size_t out_max_bytes, int *out_w, int *out_h) {
    if (!jpeg_data || size < 4 || !out_rgba || !out_w || !out_h) {
        return -1;
    }

    if (jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8) {
        return -2;
    }

    if (!g_jpeg_inited) {
        if (vita_hw_init_jpeg() < 0) {
            return -3;
        }
    }

    SceJpegOutputInfo info;
    memset(&info, 0, sizeof(info));

    int res = sceJpegGetOutputInfo(jpeg_data, (SceSize)size, 0, 0, &info);
    if (res < 0) {
        return res;
    }

    *out_w = (int)info.width;
    *out_h = (int)info.height;

    size_t required_bytes = (size_t)info.width * (size_t)info.height * 4;
    if (out_max_bytes < required_bytes) {
        return -4;
    }

#if defined(__vita__) || defined(PSP2)
    static uint8_t s_coef_buf[64 * 1024] __attribute__((aligned(64)));

    // Set sentinel bytes: valid RGBA decoded from JPEG always sets alpha channel (byte 3, 7, etc.) to 0xFF.
    // In emulators like Vita3K, sceJpegArmDecodeMJpeg is an unimplemented stub that returns 0 without writing any pixels.
    if (required_bytes >= 16) {
        memset(out_rgba, 0x55, 16);
    }

    res = sceJpegArmDecodeMJpeg(jpeg_data, (SceSize)size, 0, out_rgba, (SceSize)out_max_bytes, s_coef_buf, sizeof(s_coef_buf));
    if (res >= 0) {
        // Verify that hardware decoder actually wrote pixels (not a no-op stub)
        if (required_bytes >= 16 && (out_rgba[0] != 0x55 || out_rgba[1] != 0x55 || out_rgba[2] != 0x55 || out_rgba[3] != 0x55)) {
            return res;
        }
    }
#endif

    // Fallback to fast stb_image CPU decode (essential on Vita3K or unsupported subsamplings)
    int w = 0, h = 0, channels = 0;
    stbi_uc *pixels = stbi_load_from_memory((const stbi_uc *)jpeg_data, (int)size, &w, &h, &channels, 4);
    if (!pixels) {
        return -5;
    }
    size_t req_bytes = (size_t)w * (size_t)h * 4;
    if (out_max_bytes < req_bytes) {
        stbi_image_free(pixels);
        return -6;
    }
    memcpy(out_rgba, pixels, req_bytes);
    stbi_image_free(pixels);
    *out_w = w;
    *out_h = h;
    return 0;
}

#else

int vita_hw_init_jpeg(void) {
    return -1;
}

void vita_hw_finish_jpeg(void) {
}

int vita_decode_jpeg_hw(const uint8_t *jpeg_data, size_t size, uint8_t *out_rgba, size_t out_max_bytes, int *out_w, int *out_h) {
    (void)jpeg_data;
    (void)size;
    (void)out_rgba;
    (void)out_max_bytes;
    (void)out_w;
    (void)out_h;
    return -1;
}

#endif

int vita_decode_png_fast(const uint8_t *png_data, size_t size, uint8_t *out_rgba, size_t out_max_bytes, int *out_w, int *out_h) {
    if (!png_data || size < 8 || !out_rgba || !out_w || !out_h) {
        return -1;
    }
    if (png_data[0] != 0x89 || png_data[1] != 'P' || png_data[2] != 'N' || png_data[3] != 'G') {
        return -2;
    }
    int w = 0, h = 0, channels = 0;
    stbi_uc *pixels = stbi_load_from_memory((const stbi_uc *)png_data, (int)size, &w, &h, &channels, 4);
    if (!pixels) {
        return -3;
    }
    size_t req_bytes = (size_t)w * (size_t)h * 4;
    if (out_max_bytes < req_bytes) {
        stbi_image_free(pixels);
        return -4;
    }
    memcpy(out_rgba, pixels, req_bytes);
    stbi_image_free(pixels);
    *out_w = w;
    *out_h = h;
    return 0;
}

uint8_t *vita_decode_png_alloc(const uint8_t *png_data, size_t size, int *out_w, int *out_h) {
    if (!png_data || size < 8 || !out_w || !out_h) {
        return NULL;
    }
    if (png_data[0] != 0x89 || png_data[1] != 'P' || png_data[2] != 'N' || png_data[3] != 'G') {
        return NULL;
    }
    int w = 0, h = 0, channels = 0;
    stbi_uc *pixels = stbi_load_from_memory((const stbi_uc *)png_data, (int)size, &w, &h, &channels, 4);
    if (!pixels || w <= 0 || h <= 0) {
        if (pixels) stbi_image_free(pixels);
        return NULL;
    }
    *out_w = w;
    *out_h = h;
    return (uint8_t *)pixels;
}

void vita_free_png_pixels(uint8_t *pixels) {
    if (pixels) {
        stbi_image_free(pixels);
    }
}

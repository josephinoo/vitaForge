#include "zrif.h"
#include "puff.h"
#include <stdint.h>
#include <string.h>
#include <stdio.h>
#define ADLER32_MOD 65521
#define ZLIB_DEFLATE_METHOD 8
#define ZLIB_DICTIONARY_ID_ZRIF 0x627d1d5d
// The zlib preset dictionary NoNpDrm zRIFs are compressed against — a fixed,
// standardized 1024-byte blob (usagi-pkgj `zrif.cpp`). Every byte matters:
// this array previously held only 411 bytes, not a truncation of the same
// content but a different, corrupted sequence (silently "cleaned up" at some
// point, most likely by someone collapsing what looked like a redundant run
static const uint8_t zrif_dict[1024] = {
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,   0,   0,  48,  48,  48,  48,  57,   0,   0,   0,
          0,   0,   0,   0,   0,   0,   0,   0,  48,  48,  48,  48,
         54,  48,  48,  48,  48,  55,  48,  48,  48,  48,  56,   0,
         48,  48,  48,  48,  51,  48,  48,  48,  48,  52,  48,  48,
         48,  48,  53,  48,  95,  48,  48,  45,  65,  68,  68,  67,
         79,  78,  84,  48,  48,  48,  48,  50,  45,  80,  67,  83,
         71,  48,  48,  48,  48,  48,  48,  48,  48,  48,  48,  49,
         45,  80,  67,  83,  69,  48,  48,  48,  45,  80,  67,  83,
         70,  48,  48,  48,  45,  80,  67,  83,  67,  48,  48,  48,
         45,  80,  67,  83,  68,  48,  48,  48,  45,  80,  67,  83,
         65,  48,  48,  48,  45,  80,  67,  83,  66,  48,  48,  48,
          0,   1,   0,   1,   0,   1,   0,   2, 239, 205, 171, 137,
        103,  69,  35,   1,
};
static uint32_t adler32(const uint8_t* data, size_t size) {
    uint32_t a = 1;
    uint32_t b = 0;
    for (size_t i = 0; i < size; i++) {
        a = (a + data[i]) % ADLER32_MOD;
        b = (b + a) % ADLER32_MOD;
    }
    return (b << 16) | a;
}
static uint32_t get32be(const uint8_t* bytes) {
    return ((uint32_t)bytes[0] << 24) | ((uint32_t)bytes[1] << 16) | ((uint32_t)bytes[2] << 8) | bytes[3];
}
static uint32_t b64_decode(const char* in, uint8_t* out) {
    static const int8_t b64d[256] = {
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,62,-1,-1,-1,63,
        52,53,54,55,56,57,58,59,60,61,-1,-1,-1,-1,-1,-1,
        -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,
        15,16,17,18,19,20,21,22,23,24,25,-1,-1,-1,-1,-1,
        -1,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,
        41,42,43,44,45,46,47,48,49,50,51,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    };
    uint8_t* out0 = out;
    const uint8_t* in8 = (const uint8_t*)in;
    while (in8[0] && in8[1] && in8[2] && in8[3]) {
        int v0 = b64d[in8[0]], v1 = b64d[in8[1]], v2 = b64d[in8[2]], v3 = b64d[in8[3]];
        if (v0 < 0 || v1 < 0) break;
        *out++ = (v0 << 2) | (v1 >> 4);
        if (v2 < 0) break;
        *out++ = ((v1 & 0xF) << 4) | (v2 >> 2);
        if (v3 < 0) break;
        *out++ = ((v2 & 0x3) << 6) | v3;
        in8 += 4;
    }
    return (uint32_t)(out - out0);
}
int pkgi_zrif_decode(const char* zrif, uint8_t* rif, char* err, uint32_t err_size) {
    uint8_t raw[1024];
    uint32_t raw_len = b64_decode(zrif, raw);
    if (raw_len < 2 + 4) {
        if (err && err_size) strncpy(err, "zRIF is too short", err_size);
        return 0;
    }
    if (((raw[0] << 8) + raw[1]) % 31 != 0) {
        if (err && err_size) strncpy(err, "zRIF header is corrupted", err_size);
        return 0;
    }
    if ((raw[0] & 0xf) != ZLIB_DEFLATE_METHOD) {
        if (err && err_size) strncpy(err, "only deflate method supported in zRIF", err_size);
        return 0;
    }
    uint8_t out[1024 + sizeof(zrif_dict)];
    unsigned long dictlen = 0;
    unsigned long slen = raw_len - 4;
    const uint8_t* in = raw;
    if (raw[1] & (1 << 5)) {
        if (get32be(raw + 2) != ZLIB_DICTIONARY_ID_ZRIF) {
            if (err && err_size) strncpy(err, "zRIF uses unknown dictionary", err_size);
            return 0;
        }
        memcpy(out, zrif_dict, sizeof(zrif_dict));
        dictlen = sizeof(zrif_dict);
        in += 6;
        slen -= 6;
    } else {
        in += 2;
        slen -= 2;
    }
    unsigned long dlen = sizeof(out);
    int res = puff(dictlen, out, &dlen, in, &slen);
    if (res != 0) {
        if (err && err_size) snprintf(err, err_size, "puff decompress failed: %d", res);
        return 0;
    }
    if (dlen != 512 && dlen != 1024) {
        if (err && err_size) snprintf(err, err_size, "wrong size of zRIF: %lu", dlen);
        return 0;
    }
    memmove(out, out + dictlen, dlen);
    if (adler32(out, dlen) != get32be(in + slen)) {
        if (err && err_size) strncpy(err, "zRIF is corrupted, wrong checksum", err_size);
        return 0;
    }
    memcpy(rif, out, dlen);
    return (int)dlen;
}

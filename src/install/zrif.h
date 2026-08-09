#ifndef ZRIF_H
#define ZRIF_H
#include <stdint.h>
int pkgi_zrif_decode(const char* zrif, uint8_t* rif, char* err, uint32_t err_size);
#endif

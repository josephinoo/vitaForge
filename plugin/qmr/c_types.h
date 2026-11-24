#ifndef CAFFEINE_QMR_C_TYPES_H
#define CAFFEINE_QMR_C_TYPES_H

#include <psp2/types.h>

typedef void (*ECallback)(const char *ref_id, SceInt32 hash, SceInt32 event_id, void *user_data);
typedef void (*VoidCallback)(const char *ref_id);

typedef enum QMRWidgetType {
    button = 0,
    check_box,
    text,
    plane,
    slidebar,
    progressbar_touch,
    busyindicator
} QMRWidgetType;

typedef enum CheckBoxState {
    CHECKBOX_ON,
    CHECKBOX_OFF,
    CHECKBOX_PREV_STATE
} CheckBoxState;

#endif


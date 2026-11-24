#ifndef CAFFEINE_QM_REBORN_H
#define CAFFEINE_QM_REBORN_H

#include <psp2/types.h>
#include "c_types.h"

#define BUTTON_HANDLER(name) void name(const char *id, SceInt32 hash, SceInt32 event_id, void *user_data)
#define ONLOAD_HANDLER(name) void name(const char *id)
#define QMR_BUTTON_RELEASE_ID 0x10000008
#define SCE_PLANE_WIDTH 835.0f
#define SCE_SEPARATOR_HEIGHT 20.0f

#ifdef __cplusplus
extern "C" {
#endif

void *QuickMenuRebornRegisterWidget(const char *id, const char *parent_id, QMRWidgetType type);
int QuickMenuRebornUnregisterWidget(const char *id);
int QuickMenuRebornRegisterEventHanlder(const char *widget_id, SceInt32 event_id, ECallback function, void *user_data);
int QuickMenuRebornSeparator(const char *id, float height);
int QuickMenuRebornRemoveSeparator(const char *id);
int QuickMenuRebornSetWidgetSize(const char *id, float x, float y, float z, float w);
int QuickMenuRebornSetWidgetPosition(const char *id, float x, float y, float z, float w);
int QuickMenuRebornSetWidgetColor(const char *id, float r, float g, float b, float a);
int QuickMenuRebornSetWidgetLabel(const char *id, const char *label);
int QuickMenuRebornGetCheckboxValue(const char *id);
int QuickMenuRebornAssignOnLoadHandler(VoidCallback callback, const char *id);
int QuickMenuRebornAssignDefaultCheckBoxSave(const char *id);
int QuickMenuRebornAssignDefaultCheckBoxRecall(const char *id);
int QuickMenuRebornSaveCheckBoxState(const char *id, int state);
int QuickMenuRebornRegisterTexture(const char *id, const char *path);
int QuickMenuRebornUnregisterTexture(const char *id);
int QuickMenuRebornSetWidgetTexture(const char *id, const char *texture_id);

#ifdef __cplusplus
}
#endif

#endif

#ifndef PROXY_VITA_H
#define PROXY_VITA_H
#define PROXY_PORT 19851 /* keep in sync with PROXY_PORT in src/install/bg_jobs.rs */
int proxy_start(const char *qdir);  /* >= 0 ok */
int hook_start(const char *qdir);   /* 1 hooked, 0 unsupported firmware or failed */
void hook_stop(void);
#endif

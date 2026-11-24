#ifndef WORKER_H
#define WORKER_H
/* One pass over the queue dir: runs (or resumes) the first runnable job until it
 * finishes, fails, is paused/cancelled, or retries run out. Safe to call in a loop. */
int worker_tick(const char *qdir);   /* 1 if a job did work, 0 if idle */
/* Writes <qdir>/heartbeat with the current unix time. */
void worker_heartbeat(const char *qdir);
#endif

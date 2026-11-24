/* usage: worker_cli <qdir/> <max_ticks> [fail_net_times]; prints ticks that did work */
#include "../worker.h"
#include <stdio.h>
#include <stdlib.h>
extern int dl_fail_net_times, dl_calls;
int main(int argc, char **argv)
{
	if (argc < 3) return 99;
	if (argc > 3) dl_fail_net_times = atoi(argv[3]);
	int worked = 0;
	for (int i = 0; i < atoi(argv[2]); i++) {
		worker_heartbeat(argv[1]);
		if (!worker_tick(argv[1])) break;
		worked++;
	}
	printf("%d %d\n", worked, dl_calls);
	return 0;
}

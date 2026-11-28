/* usage: zipx_cli <zip> <dest> <overwrite 0|1> [cancel]  -> prints the zipx return code */
#include "../zipx.h"
#include <stdio.h>
#include <stdlib.h>
static int stop(uint64_t done, uint64_t total, void *user) { (void)done; (void)total; (void)user; return 1; }
int main(int argc, char **argv)
{
	if (argc < 4) return 99;
	int rc = zipx_extract(argv[1], argv[2], atoi(argv[3]), argc > 4 ? stop : NULL, NULL);
	printf("%d\n", rc);
	return 0;
}

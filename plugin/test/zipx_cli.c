/* usage: zipx_cli <zip> <dest> <overwrite 0|1>  -> prints the zipx return code */
#include "../zipx.h"
#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv)
{
	if (argc < 4) return 99;
	int rc = zipx_extract(argv[1], argv[2], atoi(argv[3]), NULL, NULL);
	printf("%d\n", rc);
	return 0;
}

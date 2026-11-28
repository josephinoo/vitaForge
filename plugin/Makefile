# Host tests for the portable core (jobs, zipx, md5). The Vita build is build.sh.
CC ?= cc
CFLAGS := -O1 -g -Wall -Wextra -fsanitize=address,undefined
LDLIBS := -lz
B := ../target/plugin-test
.PHONY: test
test:
	@mkdir -p $(B)
	$(CC) $(CFLAGS) -o $(B)/unit test/unit.c jobs.c md5.c zipx.c plat_posix.c $(LDLIBS)
	$(CC) $(CFLAGS) -o $(B)/zipx_cli test/zipx_cli.c zipx.c plat_posix.c $(LDLIBS)
	$(CC) $(CFLAGS) -o $(B)/worker_cli test/worker_cli.c worker.c jobs.c md5.c zipx.c dl_host.c plat_posix.c $(LDLIBS)
	$(CC) $(CFLAGS) -o $(B)/bgproxy test/bgproxy.c bgproxy.c jobs.c plat_posix.c
	@mkdir -p $(B)/bg && $(B)/bgproxy $(B)/bg/
	@mkdir -p $(B)/scratch && $(B)/unit $(B)/scratch
	@python3 -I test/run.py $(B)/zipx_cli
	@python3 -I test/worker.py $(B)/worker_cli

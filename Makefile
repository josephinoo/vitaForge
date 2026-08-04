.PHONY: vpk desktop ftp eboot upload-vpk run-vita vpk2
# Without this the build never sees SERVER_URL and silently falls back to the
# default catalog url baked into the source.
-include .env
export SERVER_URL
RUSTFLAGS ?= -C target-feature=-neon -A internal_features
CARGO_VITA ?= cargo +nightly vita
VPK := target/armv7-sony-vita-newlibeabihf/release/vitaforge.vpk
VITA_UPLOAD_DIR ?= ux0:/data/
DESKTOP_DIR ?= $(HOME)/Desktop
VPK_NAME := vitaforge.vpk
FTP_PORT ?= 1337
# --- Experimento: UI nativa alternativa con PocketJS (pocketjs.dev) ---
# Genera un VPK totalmente separado (title_id propio, autogenerado por su
# propio toolchain) desde experiments/pocketjs-ui/ — nunca toca $(VPK) ni
POCKETJS_DIR := experiments/pocketjs-ui
POCKETJS_APP ?= hero
VITASDK ?= /usr/local/vitasdk
vpk2:
	cd $(POCKETJS_DIR) && VITASDK="$(VITASDK)" PATH="$(VITASDK)/bin:$$PATH" bun run vita $(POCKETJS_APP) --release
	@ls -lh $(POCKETJS_DIR)/dist/vita/$(POCKETJS_APP)-main.vpk
vpk:
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO_VITA) build vpk --release
desktop: vpk
	cp $(VPK) $(DESKTOP_DIR)/vitaforge.vpk
	@ls -lh $(DESKTOP_DIR)/vitaforge.vpk
ftp: vpk
ifndef VITA_IP
	$(error Usage: make ftp VITA_IP=192.168.0.108)
endif
	curl -S --progress-bar --connect-timeout 15 --max-time 900 -T $(VPK) \
		"ftp://$(VITA_IP):$(FTP_PORT)/$(VITA_UPLOAD_DIR)$(VPK_NAME)"
	@echo "Now install ux0:/data/$(VPK_NAME) from VitaShell - copying the file does not install it."
eboot:
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO_VITA) build eboot --release

upload-vpk: vpk
ifndef VITA_IP
	$(error Usage: make upload-vpk VITA_IP=192.168.0.103)
endif
	$(CARGO_VITA) upload --vita-ip $(VITA_IP) --source $(VPK) --destination $(VITA_UPLOAD_DIR)
run-vita:
ifndef VITA_IP
	$(error Usage: make run-vita VITA_IP=192.168.0.103)
endif
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO_VITA) build eboot --update --run --vita-ip $(VITA_IP) -- --release

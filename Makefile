.PHONY: vpk desktop ftp eboot upload-vpk run-vita

RUSTFLAGS ?= -C target-feature=-neon
CARGO_VITA ?= cargo +nightly vita
VPK := target/armv7-sony-vita-newlibeabihf/release/vitaforge.vpk
VITA_UPLOAD_DIR ?= ux0:/data/
DESKTOP_DIR ?= $(HOME)/Desktop
VPK_NAME := vitaforge.vpk
FTP_PORT ?= 1337

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

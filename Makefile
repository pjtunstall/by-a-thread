# Run from the workspace root.
#
# Prerequisites: see docs/build.md and target-specific notes below.
#
# Usage:
#   make              # full build (test, server, docker, windows, deb, rpm, appimage)
#   make deploy-hetzner   # after 'make', push image to VPS and run container
#   make windows      # only Windows zip
#   make deb          # only .deb package
#   make clean        # remove dist/, temp dirs, and Docker image
#
# Make checks that required tools exist before each step, and rebuilds artifacts
# only when their dependencies have changed (e.g. Windows zip only if the exe changed).
#
.PHONY: all test server docker deploy-hetzner windows deb rpm appimage clean
.PHONY: check-windows check-deb check-rpm check-appimage check-docker check-deploy

DIST := dist
STAGING_WIN := ByAThread-win64
STAGING_APPDIR := ByAThread.AppDir
LINUXDEPLOY ?= linuxdeploy
EXE_WIN := target/x86_64-pc-windows-gnu/release/ByAThread.exe
ZIP_WIN := $(DIST)/ByAThread-win64.zip
EXE_HOST := target/release/ByAThread
APPIMAGE_FILE := $(DIST)/ByAThread.AppImage

all: test server docker windows deb rpm appimage

# --- Run tests ---
test:
	cargo test --workspace

# --- Compile game server ---
server:
	cargo build --release -p server

# --- Tool checks (run before steps that need them) ---
check-windows:
	@which x86_64-w64-mingw32-windres >/dev/null || (echo "Error: mingw-w64 not found (e.g. apt install mingw-w64)" && exit 1)
	@which zip >/dev/null || (echo "Error: zip not found" && exit 1)

check-deb:
	@cargo deb --version >/dev/null 2>&1 || (echo "Error: cargo-deb not found (cargo install cargo-deb)" && exit 1)

check-rpm:
	@cargo generate-rpm --version >/dev/null 2>&1 || (echo "Error: cargo generate-rpm not found (cargo install cargo-generate-rpm)" && exit 1)

check-appimage:
	@test -n "$$(command -v appimagetool)" || (echo "Error: appimagetool not found" && exit 1)
	@(test -x $(LINUXDEPLOY) 2>/dev/null || command -v $(LINUXDEPLOY) >/dev/null) || (echo "Error: linuxdeploy not found (set LINUXDEPLOY if needed)" && exit 1)

check-docker:
	@which docker >/dev/null || (echo "Error: docker not found" && exit 1)

check-deploy: check-docker
	@which ssh >/dev/null || (echo "Error: ssh not found" && exit 1)

# --- Build Docker image of game server ---
#
# Prerequisites: Docker (https://docs.docker.com/engine/install)
#
docker: server | check-docker
	VERSION=$$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2); \
	docker build -t server-image:$$VERSION -t server-image:latest .

# --- Update game server on VPS ---
#
# Prerequisites: VPS running; SSH access as 'hetzner'; docker in PATH on VPS
#
deploy-hetzner: docker | check-deploy
	docker save server-image | gzip | ssh hetzner 'gunzip | docker load'
	ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm -e IP=$$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'

# --- Windows executable and zip ---
#
# Prerequisites: rustup target add x86_64-pc-windows-gnu; apt install mingw-w64 zip
#
$(EXE_WIN): | check-windows
	cargo build --release --target x86_64-pc-windows-gnu -p client

$(ZIP_WIN): $(EXE_WIN)
	mkdir -p $(DIST)
	mkdir -p $(STAGING_WIN)
	cp $(EXE_WIN) $(STAGING_WIN)/
	cp LICENSE CREDITS.md $(STAGING_WIN)/
	cp client/assets/fonts/LICENSE.txt $(STAGING_WIN)/NOTO_FONT_LICENSE.txt
	zip -r $(ZIP_WIN) $(STAGING_WIN)
	rm -r $(STAGING_WIN)

windows: $(ZIP_WIN)

# --- Debian .deb package ---
#
# Prerequisites: cargo install cargo-deb
#
deb: | check-deb
	mkdir -p $(DIST)
	cd client && cargo build --release && cargo deb
	cp target/debian/by-a-thread_*.deb $(DIST)/

# --- RPM package ---
#
# Prerequisites: cargo install cargo-generate-rpm
#
rpm: | check-rpm
	mkdir -p $(DIST)
	cargo generate-rpm -p client --payload-compress gzip
	cp target/generate-rpm/*.rpm $(DIST)/

# --- AppImage ---
#
# Prerequisites: linuxdeploy (e.g. linuxdeploy-x86_64.AppImage) in PATH or set LINUXDEPLOY; appimagetool in PATH
#
$(EXE_HOST):
	cargo build --release -p client

$(APPIMAGE_FILE): $(EXE_HOST) | check-appimage
	mkdir -p $(DIST)
	rm -rf $(STAGING_APPDIR)
	mkdir -p $(STAGING_APPDIR)/usr/bin $(STAGING_APPDIR)/assets
	cp $(EXE_HOST) $(STAGING_APPDIR)/usr/bin/
	cp -r client/assets/fonts client/assets/images client/assets/sfx $(STAGING_APPDIR)/assets/
	cp client/icon.png $(STAGING_APPDIR)/ByAThread.png
	cp client/by-a-thread-appimage.desktop $(STAGING_APPDIR)/ByAThread.desktop
	bash -c '$(LINUXDEPLOY) --appdir $(STAGING_APPDIR) --executable $(STAGING_APPDIR)/usr/bin/ByAThread --desktop-file $(STAGING_APPDIR)/ByAThread.desktop --icon-file $(STAGING_APPDIR)/ByAThread.png 2>&1 | grep -v -e "WARNING: Could not find copyright" -e "AppStream upstream metadata is missing" || true; exit $${PIPESTATUS[0]}'
	bash -c 'appimagetool $(STAGING_APPDIR) $(APPIMAGE_FILE) 2>&1 | grep -v -e "WARNING: Could not find copyright" -e "AppStream upstream metadata is missing" || true; exit $${PIPESTATUS[0]}'
	rm -rf $(STAGING_APPDIR)

appimage: $(APPIMAGE_FILE)

clean:
	rm -rf $(DIST) $(STAGING_WIN) $(STAGING_APPDIR)
	-docker rmi server-image:latest $$(docker images -q server-image) 2>/dev/null || true

# Run from the workspace root.
#
# Do not run with `make -j` (parallel builds).
# See comment on unfullscreen at the end of the file.
#
# Prerequisites: see docs/build.md.
#
# If building after cloning the repo, add a `.env` file to the workspace root, of the form:
#   IP="203.0.113.42"
#   PORT="5000"
#
# Substitute the IP and port number of your default server.
#
# Usage:
#   make              # full build (test, server, docker, deb, rpm, appimage)
#   make deploy-hetzner   # after 'make', pushes image to VPS and runs container
#   make macos-intel      # Intel Mac .app and dist/ByAThread-macos-intel.zip (macOS only)
#   make macos-silicon    # Apple Silicon .app and dist/ByAThread-macos-silicon.zip (macOS only)
#   make deb          # only .deb package
#   make clean        # remove dist/, temp dirs, and Docker image
#   make fullscreen   # set fullscreen: true in client/src/main.rs (idempotent)
#
# Make checks that required tools exist before each step, and rebuilds artifacts
# only when their dependencies have changed.
#
.PHONY: all test server docker deploy-hetzner deb rpm appimage macos-intel macos-silicon clean fullscreen unfullscreen
.PHONY: check-deb check-rpm check-appimage check-docker check-deploy

DIST := dist
STAGING_APPDIR := ByAThread.AppDir
LINUXDEPLOY ?= linuxdeploy
EXE_HOST := target/release/ByAThread
APPIMAGE_FILE := $(DIST)/ByAThread.AppImage
SERVER_BIN := target/release/server
DOCKER_SENTINEL := $(DIST)/.docker-image-built
TARGET_APPLE_INTEL := x86_64-apple-darwin
TARGET_APPLE_SILICON := aarch64-apple-darwin
EXE_APPLE_INTEL := target/$(TARGET_APPLE_INTEL)/release/ByAThread
EXE_APPLE_SILICON := target/$(TARGET_APPLE_SILICON)/release/ByAThread
ZIP_APPLE_INTEL := $(DIST)/ByAThread-macos-intel.zip
ZIP_APPLE_SILICON := $(DIST)/ByAThread-macos-silicon.zip

SERVER_SOURCES := Cargo.toml Cargo.lock server/Cargo.toml common/Cargo.toml $(shell find server -name '*.rs') $(shell find common -name '*.rs')
# CLIENT_SOURCES includes .env in the workspace root. If building after cloning the repo, add a `.env` file to the workspace root, of the form:
#   IP="203.0.113.42"
#   PORT="5000"
#
# Substitute the IP and port number of your default server.
#
CLIENT_SOURCES := Cargo.toml Cargo.lock client/Cargo.toml client/build.rs $(shell find client/src -name '*.rs') common/Cargo.toml $(shell find common -name '*.rs') .env

all: test server docker deb rpm appimage unfullscreen

# Set fullscreen true in the client so the built game runs fullscreen. Only run
# this when building the client (inside those rules), not as a separate first step,
# so the source is not touched when everything is already up to date.
fullscreen:
	@grep -q 'fullscreen: false,' client/src/main.rs && sed 's|fullscreen: false,|fullscreen: true,|' client/src/main.rs > client/src/main.rs.tmp && mv client/src/main.rs.tmp client/src/main.rs || true

# --- Run tests ---
test:
	cargo test --workspace

# --- Compile game server ---
$(SERVER_BIN): $(SERVER_SOURCES)
	cargo build --release -p server

server: $(SERVER_BIN)

# --- Tool checks (run before steps that need them) ---
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
$(DOCKER_SENTINEL): $(SERVER_BIN) Dockerfile | check-docker
	mkdir -p $(DIST)
	VERSION=$$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2); \
	docker build -t server-image:$$VERSION -t server-image:latest .
	touch $(DOCKER_SENTINEL)

docker: $(DOCKER_SENTINEL)

# --- Update game server on VPS ---
#
# Prerequisites: VPS running; SSH access as 'hetzner'; docker in PATH on VPS
#
deploy-hetzner: $(DOCKER_SENTINEL) | check-deploy
	docker save server-image | gzip | ssh hetzner 'gunzip | docker load'
	ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm -e IP=$$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'
	ssh hetzner 'docker logs server-container'

# --- Debian .deb package ---
#
# Prerequisites: cargo install cargo-deb
#
$(DIST)/.deb-built: $(EXE_HOST) | check-deb
	mkdir -p $(DIST)
	@grep -q 'fullscreen: false,' client/src/main.rs && sed 's|fullscreen: false,|fullscreen: true,|' client/src/main.rs > client/src/main.rs.tmp && mv client/src/main.rs.tmp client/src/main.rs || true
	cargo deb -p client
	cp target/debian/by-a-thread_*.deb $(DIST)/
	touch $(DIST)/.deb-built

deb: $(DIST)/.deb-built
	$(MAKE) unfullscreen

# --- RPM package ---
#
# Prerequisites: cargo install cargo-generate-rpm
#
$(DIST)/.rpm-built: $(EXE_HOST) | check-rpm
	mkdir -p $(DIST)
	@grep -q 'fullscreen: false,' client/src/main.rs && sed 's|fullscreen: false,|fullscreen: true,|' client/src/main.rs > client/src/main.rs.tmp && mv client/src/main.rs.tmp client/src/main.rs || true
	cargo generate-rpm -p client --payload-compress gzip
	cp target/generate-rpm/*.rpm $(DIST)/
	touch $(DIST)/.rpm-built

rpm: $(DIST)/.rpm-built
	$(MAKE) unfullscreen

# --- AppImage ---
#
# Prerequisites: linuxdeploy (e.g. linuxdeploy-x86_64.AppImage) in PATH or set LINUXDEPLOY; appimagetool in PATH
#
$(EXE_HOST): $(CLIENT_SOURCES)
	@grep -q 'fullscreen: false,' client/src/main.rs && sed 's|fullscreen: false,|fullscreen: true,|' client/src/main.rs > client/src/main.rs.tmp && mv client/src/main.rs.tmp client/src/main.rs || true
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

# --- macOS (Intel and Apple Silicon) ---
#
# Prerequisites: run on macOS; rustup target add x86_64-apple-darwin and/or aarch64-apple-darwin; optional client/icon.icns
#
$(EXE_APPLE_INTEL): $(CLIENT_SOURCES)
	rustup target add $(TARGET_APPLE_INTEL) 2>/dev/null || true
	cargo build --release --target $(TARGET_APPLE_INTEL) -p client

$(EXE_APPLE_SILICON): $(CLIENT_SOURCES)
	rustup target add $(TARGET_APPLE_SILICON) 2>/dev/null || true
	cargo build --release --target $(TARGET_APPLE_SILICON) -p client

$(ZIP_APPLE_INTEL): $(EXE_APPLE_INTEL)
	@./scripts/bundle-macos.sh $(TARGET_APPLE_INTEL) ByAThread-macos-intel ByAThread-macos-intel.zip

$(ZIP_APPLE_SILICON): $(EXE_APPLE_SILICON)
	@./scripts/bundle-macos.sh $(TARGET_APPLE_SILICON) ByAThread-macos-silicon ByAThread-macos-silicon.zip

macos-intel: $(ZIP_APPLE_INTEL)

macos-silicon: $(ZIP_APPLE_SILICON)

clean:
	rm -rf $(DIST) $(STAGING_APPDIR) ByAThread.app
	-docker rmi server-image:latest $$(docker images -q server-image) 2>/dev/null || true

# Set fullscreen false so the source is in development state after a release build.
# Only runs when it is currently true, so the file is not touched when already false.
# Then touch client outputs so their mtime is newer than main.rs and the next make does not rebuild.
# Must be last in the Makefile so it runs after all other targets.
# This assumes `make` is not run in parallel, i.e., we should not run `make -j`.
unfullscreen:
	@grep -q 'fullscreen: true,' client/src/main.rs && sed 's|fullscreen: true,|fullscreen: false,|' client/src/main.rs > client/src/main.rs.tmp && mv client/src/main.rs.tmp client/src/main.rs || true
	@for f in $(EXE_HOST) $(DIST)/.deb-built $(DIST)/.rpm-built $(APPIMAGE_FILE); do [ -f "$$f" ] && touch "$$f"; done
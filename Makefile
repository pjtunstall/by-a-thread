# Run from the workspace root.
#
# Do not run with `make -j` (parallel builds).
#
# Prerequisites: see docs/build.md.
#
# Usage:
#   make              # full build (test, server, windows, deb, rpm, appimage)
#   make no-test      # full build without running tests
#   make test         # run tests
#   make server       # build server binary and Docker image (use docker compose build for matchmaker)
#   make deploy-hetzner   # after 'make', pushes image to VPS and runs container
#   make run-hetzner      # run server container on VPS (image must already be there)
#   make windows          # Windows zip (Ubuntu: cross-compile; Windows: use scripts/Build-Windows.ps1)
#   make macos-intel      # Intel Mac .app and dist/ByAThread-macos-intel.zip (macOS only)
#   make macos-silicon    # Apple Silicon .app and dist/ByAThread-macos-silicon.zip (macOS only)
#   make deb          # only .deb package
#   make rpm          # only .rpm package
#   make appimage     # only AppImage
#   make kill-servers # stop running game server processes and matchmaker-spawned containers
#   make clean        # remove dist/, temp dirs, and Docker images
#
# Make checks that required tools exist before each step, and rebuilds artifacts
# only when their dependencies have changed.
#
.PHONY: all no-test test server deploy-hetzner run-hetzner windows deb rpm appimage macos-intel macos-silicon clean kill-servers check-windows check-deb check-rpm check-appimage check-docker check-docker-compose check-deploy check-env

DIST := dist
STAGING_WIN := ByAThread-win64
STAGING_APPDIR := ByAThread.AppDir
EXE_WIN := target/x86_64-pc-windows-gnu/release/ByAThread.exe
ZIP_WIN := $(DIST)/ByAThread-win64.zip
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
CLIENT_SOURCES := Cargo.toml Cargo.lock client/Cargo.toml client/build.rs .env.client $(shell find client/src -name '*.rs') common/Cargo.toml $(shell find common -name '*.rs')

all: test server windows deb rpm appimage

no-test: server windows deb rpm appimage

# --- Run tests ---
test:
	cargo test --workspace

# --- Compile game server and build Docker image ---
#
# Builds the server binary and Docker image (server-image:latest).
#
$(SERVER_BIN): $(SERVER_SOURCES)
	cargo build --release -p server

server: $(DOCKER_SENTINEL)

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

check-docker-compose: check-docker
	@docker compose version >/dev/null 2>&1 || (echo "Error: docker compose not found (Docker Compose plugin required)" && exit 1)

check-deploy: check-docker
	@which ssh >/dev/null || (echo "Error: ssh not found" && exit 1)

check-env:
	@test -f .env.client || (echo "Error: .env.client required" && exit 1)
	@test -f .env.matchmaker || (echo "Error: .env.matchmaker required" && exit 1)
	@client_host=$$(grep '^HOST=' .env.client 2>/dev/null | cut -d= -f2- || echo "local"); \
	matchmaker_host=$$(grep '^HOST=' .env.matchmaker 2>/dev/null | cut -d= -f2- || echo "local"); \
	if [ "$$client_host" != "$$matchmaker_host" ]; then \
		echo "Error: HOST mismatch: .env.client has '$$client_host', .env.matchmaker has '$$matchmaker_host'"; \
		exit 1; \
	fi

# --- Build Docker image of game server (prerequisite for server target) ---
#
# Prerequisites: Docker (https://docs.docker.com/engine/install)
#
$(DOCKER_SENTINEL): $(SERVER_BIN) server/Dockerfile | check-docker
	mkdir -p $(DIST)
	VERSION=$$(cargo pkgid -p server | awk -F'[@#:]' '{print $$NF}'); \
	docker build -f server/Dockerfile -t server-image:$$VERSION -t server-image:latest .
	touch $(DOCKER_SENTINEL)

# --- Update game server on VPS ---
#
# Prerequisites: VPS running; SSH access as 'hetzner'; docker in PATH on VPS
#
deploy-hetzner: $(DOCKER_SENTINEL) | check-deploy
	docker save server-image | gzip | ssh hetzner 'gunzip | docker load'
	ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm --read-only --cap-drop ALL --security-opt no-new-privileges --cpus 0.4 --pids-limit 256 -e IP=$$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'
	ssh hetzner 'docker logs server-container'

run-hetzner: | check-deploy
	ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm --read-only --cap-drop ALL --security-opt no-new-privileges --cpus 0.4 --pids-limit 256 -e IP=$$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'
	ssh hetzner 'docker logs server-container'

# --- Windows executable and zip ---
#
# Prerequisites (Ubuntu): rustup target add x86_64-pc-windows-gnu; apt install mingw-w64 zip
# On Windows, use scripts/Build-Windows.ps1 instead.
#
$(EXE_WIN): $(CLIENT_SOURCES) | check-windows check-env
	./scripts/with-fullscreen.sh cargo build --release --target x86_64-pc-windows-gnu -p client

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
$(DIST)/.deb-built: $(EXE_HOST) | check-deb check-env
	mkdir -p $(DIST)
	./scripts/with-fullscreen.sh bash -c 'cargo deb -p client && cp target/debian/by-a-thread_*.deb $(DIST)/ && touch $(DIST)/.deb-built'

deb: $(DIST)/.deb-built

# --- RPM package ---
#
# Prerequisites: cargo install cargo-generate-rpm
#
$(DIST)/.rpm-built: $(EXE_HOST) | check-rpm check-env
	mkdir -p $(DIST)
	./scripts/with-fullscreen.sh bash -c 'cargo generate-rpm -p client --payload-compress gzip && cp target/generate-rpm/*.rpm $(DIST)/ && touch $(DIST)/.rpm-built'

rpm: $(DIST)/.rpm-built

# --- AppImage ---
#
# Prerequisites: linuxdeploy (e.g. linuxdeploy-x86_64.AppImage) in PATH or set LINUXDEPLOY; appimagetool in PATH
#
$(EXE_HOST): $(CLIENT_SOURCES) | check-env
	./scripts/with-fullscreen.sh cargo build --release -p client

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
$(EXE_APPLE_INTEL): $(CLIENT_SOURCES) | check-env
	rustup target add $(TARGET_APPLE_INTEL) 2>/dev/null || true
	./scripts/with-fullscreen.sh cargo build --release --target $(TARGET_APPLE_INTEL) -p client

$(EXE_APPLE_SILICON): $(CLIENT_SOURCES) | check-env
	rustup target add $(TARGET_APPLE_SILICON) 2>/dev/null || true
	./scripts/with-fullscreen.sh cargo build --release --target $(TARGET_APPLE_SILICON) -p client

$(ZIP_APPLE_INTEL): $(EXE_APPLE_INTEL)
	@./scripts/bundle-macos.sh $(TARGET_APPLE_INTEL) ByAThread-macos-intel ByAThread-macos-intel.zip

$(ZIP_APPLE_SILICON): $(EXE_APPLE_SILICON)
	@./scripts/bundle-macos.sh $(TARGET_APPLE_SILICON) ByAThread-macos-silicon ByAThread-macos-silicon.zip

macos-intel: $(ZIP_APPLE_INTEL)

macos-silicon: $(ZIP_APPLE_SILICON)

# Kill any running game server processes (direct run) and matchmaker-spawned containers.
kill-servers:
	-pkill -f 'target/.*/server' 2>/dev/null || true
	@containers=$$(docker ps -q --filter 'name=game-' 2>/dev/null); [ -n "$$containers" ] && echo "$$containers" | xargs docker stop 2>/dev/null || true

clean:
	rm -rf $(DIST) $(STAGING_WIN) $(STAGING_APPDIR) ByAThread.app target/debian target/generate-rpm
	cargo clean
	-docker rmi server-image:latest $$(docker images -q server-image) 2>/dev/null || true
	-docker rmi matchmaker-image:latest $$(docker images -q matchmaker-image) 2>/dev/null || true

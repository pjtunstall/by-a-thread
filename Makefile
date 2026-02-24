# Run from the workspace root.
#
# Do not run with `make -j` (parallel builds).
#
# Prerequisites: see docs/build.md.
#
# Usage:
#   make              # full build (test, server, client bundles: windows, deb, rpm, appimage)
#   make no-test      # full build without running tests
#   make test         # run tests
#   make server       # build server binary and Docker image locally
#   make build-images # build both server and matchmaker Docker images
#   make push-images  # push both Docker images to Docker Hub (uses DOCKER_USER)
#   make deploy       # build, push, update VPS config, and restart
#   make windows      # Windows zip (Ubuntu: cross-compile; Windows: use scripts/Build-Windows.ps1)
#   make macos-intel  # Intel Mac .app and dist/ByAThread-macos-intel.zip (macOS only)
#   make macos-silicon # Apple Silicon .app and dist/ByAThread-macos-silicon.zip (macOS only)
#   make deb                   # only .deb package
#   make rpm                   # only .rpm package
#   make appimage              # only AppImage
#   make kill-local-servers    # Tier 1: remove local game containers (match instances only)
#   make kill-remote-servers   # Tier 1: remove remote game containers
#   make clean-local           # Tier 2: kill local servers, remove dist/temp, cargo clean, project images
#   make clean-remote          # Tier 2: compose down on VPS, remove project images
#   make deep-clean-local      # Tier 3: clean-local + system prune (all unused containers/networks/images/volumes)
#   make deep-clean-remote     # Tier 3: clean-remote + system prune on the VPS
#
.PHONY: all no-test test server build-server-image build-matchmaker-image push-server-image push-matchmaker-image build-images push-images deploy windows deb rpm appimage macos-intel macos-silicon check-windows check-deb check-rpm check-appimage check-docker check-docker-compose check-deploy check-env kill-local-servers kill-remote-servers clean-local clean-remote deep-clean-local deep-clean-remote

DIST := dist
STAGING_WIN := ByAThread-win64
STAGING_APPDIR := ByAThread.AppDir
EXE_WIN := target/x86_64-pc-windows-gnu/release/ByAThread.exe
ZIP_WIN := $(DIST)/ByAThread-win64.zip
LINUXDEPLOY ?= linuxdeploy
EXE_HOST := target/release/ByAThread
APPIMAGE_FILE := $(DIST)/ByAThread.AppImage
SERVER_BIN := target/release/server
MATCHMAKER_BIN := target/release/matchmaker
DOCKER_SENTINEL := $(DIST)/.docker-image-built
TARGET_APPLE_INTEL := x86_64-apple-darwin
TARGET_APPLE_SILICON := aarch64-apple-darwin
EXE_APPLE_INTEL := target/$(TARGET_APPLE_INTEL)/release/ByAThread
EXE_APPLE_SILICON := target/$(TARGET_APPLE_SILICON)/release/ByAThread
ZIP_APPLE_INTEL := $(DIST)/ByAThread-macos-intel.zip
ZIP_APPLE_SILICON := $(DIST)/ByAThread-macos-silicon.zip
DOCKER_USER ?= pjtunstall

SERVER_SOURCES := Cargo.toml Cargo.lock server/Cargo.toml common/Cargo.toml $(shell find server -name '*.rs') $(shell find common -name '*.rs')
CLIENT_SOURCES := Cargo.toml Cargo.lock client/Cargo.toml client/build.rs .env.client $(shell find client/src -name '*.rs') common/Cargo.toml $(shell find common -name '*.rs')
MATCHMAKER_SOURCES := Cargo.toml Cargo.lock matchmaker/Cargo.toml common/Cargo.toml $(shell find matchmaker -name '.rs') $(shell find common -name '.rs')

all: test server windows deb rpm appimage

no-test: server windows deb rpm appimage

test:
	cargo test --workspace

# --- Tool checks ---

check-windows:
	@which x86_64-w64-mingw32-windres >/dev/null || (echo "Error: mingw-w64 not found" && exit 1)
	@which zip >/dev/null || (echo "Error: zip not found" && exit 1)

check-deb:
	@cargo deb --version >/dev/null 2>&1 || (echo "Error: cargo-deb not found" && exit 1)

check-rpm:
	@cargo generate-rpm --version >/dev/null 2>&1 || (echo "Error: cargo generate-rpm not found" && exit 1)

check-appimage:
	@test -n "$$(command -v appimagetool)" || (echo "Error: appimagetool not found" && exit 1)
	@(test -x $(LINUXDEPLOY) 2>/dev/null || command -v $(LINUXDEPLOY) >/dev/null) || (echo "Error: linuxdeploy not found" && exit 1)

check-docker:
	@which docker >/dev/null || (echo "Error: docker not found" && exit 1)

check-docker-compose: check-docker
	@docker compose version >/dev/null 2>&1 || (echo "Error: docker compose not found" && exit 1)

check-deploy: check-docker
	@which ssh >/dev/null || (echo "Error: ssh not found" && exit 1)

check-env:
	@test -f .env.client || (echo "Error: .env.client required" && exit 1)
	@test -f .env.matchmaker || (echo "Error: .env.matchmaker required" && exit 1)

# --- Docker & Server Targets ---

$(SERVER_BIN): $(SERVER_SOURCES)
	cargo build --release -p server

build-server-image: $(SERVER_BIN) server/Dockerfile | check-docker
	mkdir -p $(DIST)
	VERSION=$$(cargo pkgid -p server | awk -F'[@#:]' '{print $$NF}'); \
	docker build -f server/Dockerfile -t $(DOCKER_USER)/server-image:$$VERSION -t $(DOCKER_USER)/server-image:latest .
	touch $(DOCKER_SENTINEL)

server: build-server-image

$(MATCHMAKER_BIN): $(MATCHMAKER_SOURCES)
	cargo build --release -p matchmaker

build-matchmaker-image: $(MATCHMAKER_BIN) matchmaker/Dockerfile | check-docker
	mkdir -p $(DIST)
	VERSION=$$(cargo pkgid -p matchmaker | awk -F'[@#:]' '{print $$NF}'); \
	docker build -f matchmaker/Dockerfile -t $(DOCKER_USER)/matchmaker-image:$$VERSION -t $(DOCKER_USER)/matchmaker-image:latest .

build-images: build-server-image build-matchmaker-image

push-server-image: build-server-image
	docker push $(DOCKER_USER)/server-image:latest
	VERSION=$$(cargo pkgid -p server | awk -F'[@#:]' '{print $$NF}'); \
	docker push $(DOCKER_USER)/server-image:$$VERSION

push-matchmaker-image: build-matchmaker-image
	docker push $(DOCKER_USER)/matchmaker-image:latest
	VERSION=$$(cargo pkgid -p matchmaker | awk -F'[@\#:]' '{print $$NF}'); \
	docker push $(DOCKER_USER)/matchmaker-image:$$VERSION

push-images: push-server-image push-matchmaker-image

# --env-file .env.matchmaker tells the Docker Compose CLI to use .env.matchmaker
# to resolve YAML variables (like ${GAME_IMAGE}) before pulling/starting.
# Separately, the 'env_file' directive inside docker-compose.yaml tells the
# container to load its internal environment variables from that same file.
# Docker Compose then augments the variables from the file with others that it
# specifies in the 'environment' directive.
deploy: | check-deploy
	scp docker-compose.yaml Caddyfile .env.matchmaker $(HOST):~/
	ssh $(HOST) 'docker compose --env-file .env.matchmaker pull && \
		docker pull $(DOCKER_USER)/server-image:latest && \
		docker compose --env-file .env.matchmaker up -d'

# --- Client Build Targets ---

$(EXE_WIN): $(CLIENT_SOURCES) | check-windows check-env
	./scripts/with-fullscreen.sh cargo build --release --target x86_64-pc-windows-gnu -p client

$(ZIP_WIN): $(EXE_WIN)
	mkdir -p $(DIST)
	mkdir -p $(STAGING_WIN)
	cp $(EXE_WIN) $(STAGING_WIN)/
	cp LICENSE CREDITS $(STAGING_WIN)/
	cp client/assets/fonts/LICENSE.txt $(STAGING_WIN)/NOTO_FONT_LICENSE.txt
	zip -r $(ZIP_WIN) $(STAGING_WIN)
	rm -r $(STAGING_WIN)

windows: $(ZIP_WIN)

$(DIST)/.deb-built: $(EXE_HOST) | check-deb check-env
	mkdir -p $(DIST)
	./scripts/with-fullscreen.sh bash -c 'cargo deb -p client && cp target/debian/by-a-thread_*.deb $(DIST)/ && touch $(DIST)/.deb-built'

deb: $(DIST)/.deb-built

$(DIST)/.rpm-built: $(EXE_HOST) | check-rpm check-env
	mkdir -p $(DIST)
	./scripts/with-fullscreen.sh bash -c 'cargo generate-rpm -p client --payload-compress gzip && cp target/generate-rpm/*.rpm $(DIST)/ && touch $(DIST)/.rpm-built'

rpm: $(DIST)/.rpm-built

$(EXE_HOST): $(CLIENT_SOURCES) | check-env
	./scripts/with-fullscreen.sh cargo build --release -p client

$(APPIMAGE_FILE): $(EXE_HOST) | check-appimage
	mkdir -p $(DIST)
	rm -rf $(STAGING_APPDIR)
	mkdir -p $(STAGING_APPDIR)/usr/bin $(STAGING_APPDIR)/assets
	cp $(EXE_HOST) $(STAGING_APPDIR)/usr/bin/
	cp -r client/assets/fonts client/assets/images client/assets/sfx $(STAGING_APPDIR)/assets/
	cp client/icon.png $(STAGING_APPDIR)/by-a-thread.png
	cp client/by-a-thread-appimage.desktop $(STAGING_APPDIR)/by-a-thread-appimage.desktop
	bash -c '$(LINUXDEPLOY) --appdir $(STAGING_APPDIR) --executable $(STAGING_APPDIR)/usr/bin/ByAThread --desktop-file $(STAGING_APPDIR)/by-a-thread-appimage.desktop --icon-file $(STAGING_APPDIR)/by-a-thread.png 2>&1 | grep -v -e "WARNING: Could not find copyright" -e "AppStream upstream metadata is missing" || true; exit $${PIPESTATUS[0]}'
	bash -c 'appimagetool $(STAGING_APPDIR) $(APPIMAGE_FILE) 2>&1 | grep -v -e "WARNING: Could not find copyright" -e "AppStream upstream metadata is missing" || true; exit $${PIPESTATUS[0]}'
	rm -rf $(STAGING_APPDIR)

appimage: $(APPIMAGE_FILE)

macos-intel: 
	@./scripts/bundle-macos.sh $(TARGET_APPLE_INTEL) ByAThread-macos-intel ByAThread-macos-intel.zip

macos-silicon: 
	@./scripts/bundle-macos.sh $(TARGET_APPLE_SILICON) ByAThread-macos-silicon ByAThread-macos-silicon.zip

# --- Tier 1: Instance cleanup (match instances only) ---

kill-local-servers:
	@containers=$$(docker ps -aq --filter "name=game-" 2>/dev/null); \
	if [ -n "$$containers" ]; then \
		echo "Removing local game containers:"; \
		docker ps -a --filter "name=game-" --format "table {{.Names}}\t{{.CreatedAt}}\t{{.Status}}"; \
		echo "$$containers" | xargs docker rm -f >/dev/null; \
	else \
		echo "No local game servers to clean up."; \
	fi

kill-remote-servers: | check-deploy
	@ssh hetzner "containers=\$$(docker ps -aq --filter 'name=game-' 2>/dev/null); \
	if [ -n \"\$$containers\" ]; then \
		echo \"Removing remote game containers:\"; \
		docker ps -a --filter \"name=game-\" --format \"table {{.Names}}\t{{.CreatedAt}}\t{{.Status}}\"; \
		echo \"\$$containers\" | xargs docker rm -f >/dev/null; \
	else \
		echo \"No remote game servers to clean up.\"; \
	fi"

# --- Tier 2: Project cleanup (binaries and images) ---

clean-local: kill-local-servers
	rm -rf $(DIST) $(STAGING_WIN) $(STAGING_APPDIR) ByAThread.app target/debian target/generate-rpm
	cargo clean
	-docker rmi $$(docker images -q $(DOCKER_USER)/*-image) 2>/dev/null || true
	docker image prune -f
	docker builder prune -f

clean-remote: | check-deploy
	ssh hetzner "docker compose down; \
	docker rmi \$$(docker images -q $(DOCKER_USER)/*-image) 2>/dev/null || true; \
	docker image prune -f"

# --- Tier 3: Nuclear cleanup (system reset) ---

deep-clean-local: clean-local
	docker system prune -af --volumes

deep-clean-remote: clean-remote
	ssh hetzner "docker system prune -af --volumes"
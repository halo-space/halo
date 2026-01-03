.PHONY: build mac win linux image clean

# ---- Configurable variables ----
CRATE ?= rsctl
BIN ?= rsctl
DIST ?= dist

# Host info
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)

# macOS target: default to host arch
# - Apple Silicon: aarch64-apple-darwin
# - Intel:         x86_64-apple-darwin
MAC_TARGET := $(if $(filter arm64,$(UNAME_M)),aarch64-apple-darwin,x86_64-apple-darwin)

# Read RS generator version from src/version.rs (pub const VERSION: &str = "x.y";)
VERSION ?= $(shell rg -N --no-filename --replace '$$1' 'pub const VERSION: &str = "([^"]+)";' src/version.rs 2>/dev/null | head -n 1)

# Docker image name can be overridden:
#   make image IMAGE=yourname/rsctl
IMAGE ?= rsctl

build:
	cargo build -p $(CRATE) --release --manifest-path ../Cargo.toml
	mkdir -p $(DIST)
	cp ../target/release/$(BIN) $(DIST)/$(BIN)

mac:
	# Build for current macOS host arch by default.
	# If you need Intel binary on Apple Silicon: `make mac-amd64` (requires: rustup target add x86_64-apple-darwin)
	cargo build -p $(CRATE) --release --target $(MAC_TARGET) --manifest-path ../Cargo.toml
	mkdir -p $(DIST)
	cp ../target/$(MAC_TARGET)/release/$(BIN) $(DIST)/$(BIN)

mac-amd64:
	cargo build -p $(CRATE) --release --target x86_64-apple-darwin --manifest-path ../Cargo.toml
	mkdir -p $(DIST)
	cp ../target/x86_64-apple-darwin/release/$(BIN) $(DIST)/$(BIN)-darwin-amd64

mac-arm64:
	cargo build -p $(CRATE) --release --target aarch64-apple-darwin --manifest-path ../Cargo.toml
	mkdir -p $(DIST)
	cp ../target/aarch64-apple-darwin/release/$(BIN) $(DIST)/$(BIN)-darwin-arm64

win:
	# Note: building windows binaries on macOS requires a suitable Rust target + linker toolchain.
	# Common target: x86_64-pc-windows-gnu (needs mingw-w64).
	cargo build -p $(CRATE) --release --target x86_64-pc-windows-gnu --manifest-path ../Cargo.toml
	mkdir -p $(DIST)
	cp ../target/x86_64-pc-windows-gnu/release/$(BIN).exe $(DIST)/$(BIN).exe

linux:
	# Note: building linux binaries on macOS requires a suitable Rust target + linker toolchain.
	# Common target: x86_64-unknown-linux-gnu.
	cargo build -p $(CRATE) --release --target x86_64-unknown-linux-gnu --manifest-path ../Cargo.toml
	mkdir -p $(DIST)
	cp ../target/x86_64-unknown-linux-gnu/release/$(BIN) $(DIST)/$(BIN)-linux

image:
	# Build & tag linux/amd64 image
	docker build --rm --platform linux/amd64 -t $(IMAGE):$(VERSION) .
	docker tag $(IMAGE):$(VERSION) $(IMAGE):latest
	# Build & tag linux/arm64 image
	docker build --rm --platform linux/arm64 -t $(IMAGE):$(VERSION)-arm64 .
	docker tag $(IMAGE):$(VERSION)-arm64 $(IMAGE):latest-arm64
	# Push (optional): uncomment if you want push-by-default
	# docker push $(IMAGE):$(VERSION)
	# docker push $(IMAGE):latest
	# docker push $(IMAGE):$(VERSION)-arm64
	# docker push $(IMAGE):latest-arm64

clean:
	rm -rf $(DIST)
	cargo clean



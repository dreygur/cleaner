BINARY := cleaner
PREFIX ?= $(HOME)/.local/bin
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)

APPDIR ?= /Applications

.PHONY: build test lint install clean scan ui package install-app go-build go-test compare

build:
	cargo build --release

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets

# The window runs the engine binary that sits next to it.
ui:
	cargo build --release --workspace
	./target/release/cleaner-ui

install: build
	mkdir -p $(PREFIX)
	install -m 0755 target/release/$(BINARY) $(PREFIX)/$(BINARY)

# dist/Cleaner.app, the release zip and the Homebrew cask for it.
package:
	scripts/package.sh

# A locally built app carries no quarantine flag, so it opens without the
# Gatekeeper prompt a downloaded unsigned app would get.
install-app: package
	rm -rf $(APPDIR)/Cleaner.app
	ditto dist/Cleaner.app $(APPDIR)/Cleaner.app

scan: build
	./target/release/$(BINARY) --dry-run

go-build:
	cd go && go build -ldflags "-s -w -X main.appVersion=$(VERSION)" -o ../bin/$(BINARY)-go .

go-test:
	cd go && go test ./...

compare: build go-build
	scripts/compare.sh bin/$(BINARY)-go target/release/$(BINARY)

clean:
	rm -rf bin target dist

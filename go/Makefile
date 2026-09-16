BINARY := cleaner
PREFIX ?= $(HOME)/.local/bin
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)

.PHONY: build test vet install clean scan

build:
	go build -ldflags "-s -w -X main.appVersion=$(VERSION)" -o bin/$(BINARY) .

test:
	go test ./...

vet:
	go vet ./...

install: build
	mkdir -p $(PREFIX)
	install -m 0755 bin/$(BINARY) $(PREFIX)/$(BINARY)

scan: build
	./bin/$(BINARY) --dry-run

clean:
	rm -rf bin

.PHONY: build test vet install clean scan

build:
	$(MAKE) -C go build

test:
	$(MAKE) -C go test

vet:
	$(MAKE) -C go vet

install:
	$(MAKE) -C go install

clean:
	$(MAKE) -C go clean

scan:
	$(MAKE) -C go scan

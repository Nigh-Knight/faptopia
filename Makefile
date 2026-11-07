BIN     := faptopia
INSTALL := $(HOME)/.local/bin/$(BIN)

.DEFAULT_GOAL := build
.PHONY: build check clippy fmt test run install clean

# Compile and put it on PATH. Never leave the installed binary stale.
build:
	cargo build --release
	install -Dm755 target/release/$(BIN) $(INSTALL)
	@echo "installed -> $(INSTALL)"

install: build

check:
	cargo check

# Pre-existing warnings (ureq's large Err variants) keep this off -D warnings.
clippy:
	cargo clippy

fmt:
	cargo fmt

test:
	cargo test
	node tests/script_resilience.test.js

# make run BOARDS="r/x:top:week r/y:top:all"
run: build
	$(BIN) reddit --limit 25 $(BOARDS)

clean:
	cargo clean
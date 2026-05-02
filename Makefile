SHELL := bash
CARGO := cargo
CARGOFLAGS ?= --workspace

.PHONY: all ci fmt fmt-check lint check test

all: ci

# Format code in-place
fmt:
	$(CARGO) fmt --all

# Verify formatting (no changes)
fmt-check:
	$(CARGO) fmt --all -- --check

# Lint with Clippy; treat warnings as errors (incl. rustc warnings)
lint:
	RUSTFLAGS="-D warnings" $(CARGO) clippy $(CARGOFLAGS) --all-targets -- -D warnings

# Fast type-check of the workspace
check:
	$(CARGO) check $(CARGOFLAGS) --all-targets

# Run tests
test:
	$(CARGO) test $(CARGOFLAGS) --all-targets

# CI entry: verify format and lint
ci: fmt-check lint

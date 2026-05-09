SHELL := bash
CARGO := cargo
CARGOFLAGS ?=

.PHONY: all ci fmt fmt-check lint check test feature-check

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

# Fast type-check
check:
	$(CARGO) check $(CARGOFLAGS) --all-targets

# Run tests
test:
	$(CARGO) test $(CARGOFLAGS) --all-targets

# Verify supported feature combinations compile cleanly
feature-check:
	RUSTFLAGS="-D warnings" $(CARGO) clippy --no-default-features --all-targets -- -D warnings
	RUSTFLAGS="-D warnings" $(CARGO) clippy --no-default-features --features bmp --all-targets -- -D warnings
	RUSTFLAGS="-D warnings" $(CARGO) clippy --no-default-features --features netpbm --all-targets -- -D warnings

# CI entry: verify format, lint, and tests
ci: fmt-check lint test feature-check

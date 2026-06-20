# ─────────────────────────────────────────────────────────────────────────────
# globalping-probe — Rust rewrite
# Toolchain : stable-x86_64-pc-windows-gnu  (GNU linker, no MSVC needed)
# Default   : make help
# ─────────────────────────────────────────────────────────────────────────────

CARGO        := $(USERPROFILE)/.cargo/bin/cargo.exe
LINUX_TARGET := x86_64-unknown-linux-musl
ARM_TARGET   := aarch64-unknown-linux-musl

.DEFAULT_GOAL := help

.PHONY: all help fmt fmt-check lint check build build-release run doc \
        test test-unit test-integration test-verbose \
        ci cross-linux cross-arm clean

# ── Help ─────────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo "  globalping-probe (Rust)"
	@echo ""
	@echo "  Code quality"
	@echo "    make fmt            format all source (rustfmt)"
	@echo "    make fmt-check      check formatting without modifying"
	@echo "    make lint           clippy — warnings are hard errors"
	@echo "    make check          type-check only (fast, no binary)"
	@echo ""
	@echo "  Build"
	@echo "    make build          debug build"
	@echo "    make build-release  release build (optimized + stripped)"
	@echo "    make run            run in debug mode"
	@echo "    make doc            generate and open rustdoc"
	@echo ""
	@echo "  Test"
	@echo "    make test           all tests (unit + integration)"
	@echo "    make test-unit      unit tests only"
	@echo "    make test-integration  integration tests only"
	@echo "    make test-verbose   all tests with stdout visible"
	@echo ""
	@echo "  Cross-compile (Linux)"
	@echo "    make cross-linux    x86_64-unknown-linux-musl"
	@echo "    make cross-arm      aarch64-unknown-linux-musl"
	@echo ""
	@echo "  Pipeline"
	@echo "    make ci             fmt-check + lint + test  (CI gate)"
	@echo "    make all            fmt + lint + build + test"
	@echo ""
	@echo "  Misc"
	@echo "    make clean          remove build artifacts"
	@echo ""

# ── Code quality ─────────────────────────────────────────────────────────────

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

check:
	$(CARGO) check --all-targets

# ── Build ────────────────────────────────────────────────────────────────────

build:
	$(CARGO) build

build-release:
	$(CARGO) build --release

run:
	$(CARGO) run

doc:
	$(CARGO) doc --no-deps --open

# ── Test ─────────────────────────────────────────────────────────────────────

test:
	$(CARGO) test --all

test-unit:
	$(CARGO) test --lib

test-integration:
	$(CARGO) test --tests

test-verbose:
	$(CARGO) test --all -- --nocapture

# ── Cross-compilation ────────────────────────────────────────────────────────

cross-linux:
	$(CARGO) build --release --target $(LINUX_TARGET)

cross-arm:
	$(CARGO) build --release --target $(ARM_TARGET)

# ── Pipeline ─────────────────────────────────────────────────────────────────

ci: fmt-check lint test

all: fmt lint build test

# ── Misc ─────────────────────────────────────────────────────────────────────

clean:
	$(CARGO) clean

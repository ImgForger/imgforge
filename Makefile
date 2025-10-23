# Default target
.PHONY: all
all: build

# Build the project
.PHONY: build
build:
	cargo build

# Clean the project
.PHONY: clean
clean:
	cargo clean

# Run tests
.PHONY: test
test:
	cargo test --all -- --test-threads=1

# Format the code
.PHONY: format
format:
	cargo fmt

# Update dependencies
.PHONY: update
update:
	cargo update

# Upgrade dependencies
.PHONY: upgrade
upgrade:
	cargo-upgrade upgrade

# Lint the code
.PHONY: lint
lint:
	cargo clippy -- -D warnings

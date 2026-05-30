title := $(shell tput setaf 4)$(shell tput bold)
sgr0 := $(shell tput sgr0)
sep := $(shell tput setaf 4)===============================$(sgr0)
RUST_DIR = rust

.PHONY: all build coverage format lint lint-md test

# Run all checks. Coverage includes tests, so "test" is not a separate step here.
all: coverage lint lint-md format build echo

build:
	@echo
	@echo "$(title)Building Rust code...$(sgr0)"
	@echo "$(sep)"
	cd $(RUST_DIR) && cargo build

coverage:
	@echo
	@printf "$(title)Running tests with coverage...$(sgr0)\n"
	@echo "$(sep)"
	cd $(RUST_DIR) && cargo llvm-cov --workspace --all-targets --summary-only --fail-under-functions 100 --fail-under-lines 100 --fail-under-regions 100

echo:
	@echo

format:
	@echo
	@echo "$(title)Checking code formatting...$(sgr0)"
	@echo "$(sep)"
	cd $(RUST_DIR) && cargo fmt -- --check

lint:
	@echo
	@echo "$(title)Running clippy linter...$(sgr0)"
	@echo "$(sep)"
	cd $(RUST_DIR) && cargo clippy --all-targets -- -D warnings

test:
	@echo
	@echo "$(title)Running tests...$(sgr0)"
	@echo "$(sep)"
	cd $(RUST_DIR) && cargo test

lint-md:
	@echo
	@echo "$(title)Running Markdown link checks...$(sgr0)"
	@echo "$(sep)"
	./scripts/lint-md

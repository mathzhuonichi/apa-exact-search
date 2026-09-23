.DEFAULT_GOAL := help
.NOTPARALLEL:
CXX ?= c++
CXXFLAGS ?= -O3 -std=c++17 -Wall -Wextra

.PHONY: help check syntax build rust-build
help:
	@echo "Paused research snapshot. Safe commands: make check; make rust-build."
	@echo "Read docs/STATUS.md before considering any search."
check:
	cargo fmt --check
	cargo check
	cargo test
syntax:
	$(CXX) -std=c++17 -fsyntax-only src/fixed_max_factor_branch_v11_20260920.cpp
	$(CXX) -std=c++17 -fsyntax-only src/fixed_max_unary_propagation_20260920.cpp
	$(CXX) -std=c++17 -fsyntax-only src/fixed_max_factor_branch_v9_20260920.cpp
build: build/solver build/root-propagator build/baseline
rust-build:
	cargo build --release
build/solver: src/fixed_max_factor_branch_v11_20260920.cpp
	mkdir -p build
	$(CXX) $(CXXFLAGS) $< -o $@
build/root-propagator: src/fixed_max_unary_propagation_20260920.cpp
	mkdir -p build
	$(CXX) $(CXXFLAGS) $< -o $@
build/baseline: src/fixed_max_factor_branch_v9_20260920.cpp
	mkdir -p build
	$(CXX) $(CXXFLAGS) $< -o $@

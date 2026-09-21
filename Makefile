.DEFAULT_GOAL := help
.NOTPARALLEL:
CXX ?= c++
CXXFLAGS ?= -O3 -std=c++17 -Wall -Wextra

.PHONY: help check syntax build
help:
	@echo "Paused research snapshot. Safe commands: make check; make build (compile only)."
	@echo "Read docs/STATUS.md before considering any search."
check:
	python3 tools/check_snapshot.py
syntax:
	$(CXX) -std=c++17 -fsyntax-only src/fixed_max_factor_branch_v11_20260920.cpp
	$(CXX) -std=c++17 -fsyntax-only src/fixed_max_unary_propagation_20260920.cpp
	$(CXX) -std=c++17 -fsyntax-only src/fixed_max_factor_branch_v9_20260920.cpp
build: build/solver build/root-propagator build/baseline
build/solver: src/fixed_max_factor_branch_v11_20260920.cpp
	mkdir -p build
	$(CXX) $(CXXFLAGS) $< -o $@
build/root-propagator: src/fixed_max_unary_propagation_20260920.cpp
	mkdir -p build
	$(CXX) $(CXXFLAGS) $< -o $@
build/baseline: src/fixed_max_factor_branch_v9_20260920.cpp
	mkdir -p build
	$(CXX) $(CXXFLAGS) $< -o $@

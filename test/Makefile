
CC ?= cc
SRC = ../src/non-layered-tidy-trees.c
WARNINGS = -Wall -Wextra -Wshadow
CFLAGS ?= -std=c11 -O1 -g $(WARNINGS)

TESTS = overlap regression properties callbacks flatapi depth

# Trees per shape for the sweep and the golden dump.  The full corpus is 115200 trees and
# about 60 MB of coordinates; `make golden TRIALS=10` is a couple of MB.
TRIALS ?= 300
GOLDEN = golden.txt

# The benchmark is built with the flags the shipped library uses, not the `-O1` of the
# tests, so that the number means something; `make bench BENCH_CFLAGS=-O3` to compare.
BENCH_CFLAGS ?= -std=c11 -O2 $(WARNINGS)

.PHONY: check all asan golden check-golden wasm bench clean

# The default run: every test, stopping at the first failure.
check: all
	@status=0; \
	for t in $(TESTS); do \
		echo "== $$t"; ./$$t || status=1; \
	done; \
	exit $$status

all: $(TESTS)

$(TESTS): %: %.c treegen.h $(SRC)
	$(CC) $(CFLAGS) -o $@ $< $(SRC)

# Same suite under the sanitizers; `layout_api` and `free_tree` must be leak clean.
asan:
	@status=0; \
	for t in $(TESTS); do \
		$(CC) $(CFLAGS) -fsanitize=address,undefined -o $$t.san $$t.c $(SRC) || exit 1; \
		echo "== $$t (asan+ubsan)"; ASAN_OPTIONS=detect_leaks=1 ./$$t.san || status=1; \
	done; \
	exit $$status

# `overlap --dump` over the whole corpus is a differential oracle: freeze it with
# `make golden`, then `make check-golden` after a change.  A tidy refactoring leaves it
# byte for byte identical; a translation of the drawing shows up as a constant offset.
golden: overlap
	./overlap --dump $(TRIALS) > $(GOLDEN)
	@echo "wrote $(GOLDEN) ($$(wc -l < $(GOLDEN)) lines)"

check-golden: overlap
	@./overlap --dump $(TRIALS) > $(GOLDEN).new
	@if diff -q $(GOLDEN) $(GOLDEN).new >/dev/null; then \
		echo "PASS  coordinates identical to $(GOLDEN)"; rm -f $(GOLDEN).new; \
	else \
		echo "FAIL  coordinates differ from $(GOLDEN); see $(GOLDEN).new"; exit 1; \
	fi

# Needs `make -C ../src emscripten` first.
wasm:
	node wasm_smoke.js

# One large tree, the workload `simd-plan.md` scopes for; `rust/src/bin/bench.rs` prints
# the same table for the Rust implementations, over the very same trees.
bench: bench.c treegen.h $(SRC)
	$(CC) $(BENCH_CFLAGS) -o bench bench.c $(SRC)
	./bench

clean:
	rm -f $(TESTS) bench *.san $(GOLDEN) $(GOLDEN).new
	rm -rf *.dSYM

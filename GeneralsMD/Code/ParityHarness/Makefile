CXX ?= c++
CXXFLAGS ?= -std=c++17 -O2 -Wall -Wextra -Wpedantic

ROOT := $(abspath $(CURDIR)/../../..)
SHIMS := $(CURDIR)/shims
BIN := $(CURDIR)/bin/generalsmd_frame_trace
SCENARIO ?= $(ROOT)/parity_scenarios/smoke_attack.v1.json
SHIM_HEADERS := $(shell find "$(SHIMS)" -type f -name '*.h')

.PHONY: all clean test

all: $(BIN)

$(BIN): main.cpp original_random_adapter.cpp $(SHIM_HEADERS)
	@mkdir -p $(dir $@)
	$(CXX) $(CXXFLAGS) -D_DEBUG -I$(SHIMS) -o $@ main.cpp original_random_adapter.cpp

test: $(BIN)
	@tmp=$$(mktemp); \
	$(BIN) $(SCENARIO) > $$tmp; \
	python3 tests/validate_trace.py $$tmp $(SCENARIO); \
	rm -f $$tmp

clean:
	rm -rf bin

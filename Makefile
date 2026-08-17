CARGO ?= cargo
SEED  ?= 42
TICKS ?= 50000
OUT   ?= runs/run.csv

.PHONY: build run sim test lint fmt ci clean

build:
	$(CARGO) build --workspace

## Open the viewer (native window)
run:
	$(CARGO) run --release -p vivere-viewer

## Headless run: make sim SEED=42 TICKS=50000 OUT=runs/run.csv
sim:
	@mkdir -p runs
	$(CARGO) run --release -p vivere-cli -- run --seed $(SEED) --ticks $(TICKS) --out $(OUT)

test:
	$(CARGO) test --workspace

lint:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings

fmt:
	$(CARGO) fmt --all

ci: lint test

clean:
	$(CARGO) clean

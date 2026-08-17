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

## Watch a world in the terminal: make tty [TTY_ARGS="--resume runs/x.snap"]
TTY_ARGS ?= --seed 42
tty:
	$(CARGO) run --release -p vivere-tty -- $(TTY_ARGS)

## Headless run: make sim SEED=42 TICKS=50000 OUT=runs/run.csv
sim:
	@mkdir -p runs
	$(CARGO) run --release -p vivere-cli -- run --seed $(SEED) --ticks $(TICKS) --out $(OUT)

## Render the README GIF headlessly: make gif [GIF_ARGS="--resume runs/x.snap ..."]
GIF_ARGS ?= --seed 42 --ticks 4000 --every 8 --out docs/assets/protocell.gif
gif:
	$(CARGO) run --release -p vivere-gif -- $(GIF_ARGS)

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

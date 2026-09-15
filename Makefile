# Typical development actions for the ymp repository.
# The canonical commands live in AGENTS.md and CONTRIBUTING.md; this file only wraps them.
# Run `make help` to list targets. Override CARGO_FLAGS= to allow network access.

CARGO_FLAGS ?= --offline
TASKS := python3 ymp-docs/tasks/manage.py
ID ?=
OWNER ?= $(USER)
REV ?=
NOTE ?=
ARGS ?=

.DEFAULT_GOAL := help
.PHONY: help build fmt fmt-check clippy test doc verify run clean legacy-scan \
        tasks-check tasks-next tasks-list tasks-show tasks-deps tasks-history tasks-summary tasks-render \
        tasks-claim tasks-status tasks-test all-checks

help: ## List targets
	@printf 'Targets:\n'
	@grep -E '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-16s %s\n", $$1, $$2}'
	@printf '\nVariables: CARGO_FLAGS=%s ID=<task> OWNER=<name> REV=<revision> NOTE=<text> ARGS=<cli args>\n' "$(CARGO_FLAGS)"

# ---- Cargo (the four pre-commit checks from AGENTS.md) ----

build: ## Build every crate
	cargo build --workspace $(CARGO_FLAGS)

fmt: ## Format the workspace in place
	cargo fmt --all

fmt-check: ## Fail if formatting differs
	cargo fmt --all --check

clippy: ## Lint with warnings as errors
	cargo clippy --workspace --all-targets $(CARGO_FLAGS) -- -D warnings

test: ## Run all tests
	cargo test --workspace $(CARGO_FLAGS)

doc: ## Build API documentation
	cargo doc --workspace --no-deps $(CARGO_FLAGS)

legacy-scan: ## Reject legacy identifiers and blocks copied from legacy-* tags
	python3 tools/legacy_scan.py

verify: legacy-scan build fmt-check clippy test ## Legacy scan plus the four required checks in AGENTS.md order
	@printf 'verify: legacy scan and the four pre-commit checks passed\n'

run: ## Run the ymp executable, e.g. make run ARGS='--help'
	cargo run -p ymp-cli $(CARGO_FLAGS) -- $(ARGS)

clean: ## Remove build artifacts
	cargo clean

# ---- Development task register (ymp-docs/tasks/manage.py) ----

tasks-check: ## Validate the task register
	$(TASKS) check

tasks-next: ## Show the next ready task, e.g. make tasks-next ARGS='--wave W1 --limit 3'
	$(TASKS) next $(ARGS)

tasks-list: ## List tasks, e.g. make tasks-list ARGS='--wave W1 --readiness ready'
	$(TASKS) list $(ARGS)

tasks-show: ## Show one task: make tasks-show ID=W1-0001
	@test -n "$(ID)" || { echo 'usage: make tasks-show ID=W1-0001'; exit 2; }
	$(TASKS) show $(ID)

tasks-deps: ## Show transitive dependencies of a task
	@test -n "$(ID)" || { echo 'usage: make tasks-deps ID=W1-0014'; exit 2; }
	$(TASKS) deps $(ID)

tasks-history: ## Show the history of a task
	@test -n "$(ID)" || { echo 'usage: make tasks-history ID=W1-0001'; exit 2; }
	$(TASKS) history $(ID)

tasks-summary: ## Status counts by wave, area and type
	$(TASKS) summary

tasks-render: ## Bounded Markdown overview on stdout
	$(TASKS) render --limit 20

tasks-claim: ## Claim a ready task: make tasks-claim ID=W1-0001 REV=4
	@test -n "$(ID)" && test -n "$(REV)" || { echo 'usage: make tasks-claim ID=W1-0001 REV=4 [OWNER=name]'; exit 2; }
	$(TASKS) claim $(ID) --owner $(OWNER) --expect-revision $(REV)

tasks-status: ## Record a status change: make tasks-status ID=… STATE=… REV=… NOTE='…'
	@test -n "$(ID)" && test -n "$(REV)" && test -n "$(STATE)" && test -n "$(NOTE)" || { echo 'usage: make tasks-status ID=W1-0001 STATE=in_progress REV=5 NOTE="..." [OWNER=actor]'; exit 2; }
	$(TASKS) status $(ID) $(STATE) --expect-revision $(REV) --actor $(OWNER) --note "$(NOTE)"

tasks-test: ## Test the task tool itself (required before changing the workflow)
	python3 -m unittest discover -s ymp-docs/tasks/tests -v
	$(TASKS) render --limit 20 >/dev/null

# ---- Everything CONTRIBUTING.md asks for before submitting a change ----

all-checks: tasks-test tasks-check verify ## Task-tool tests, register check and the four Cargo checks
	@printf 'all-checks: task register and workspace checks passed\n'

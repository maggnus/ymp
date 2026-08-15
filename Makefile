# Build and install the ymp executable.
#
#   make install              bump the patch version, build the release binary, install to ~/.local/bin
#   make install PREFIX=/usr/local
#   make build                build without bumping
#   make version              print the current version
#   make uninstall
#
# The version lives in one place — [workspace.package] version in ymp-rust/Cargo.toml — and every
# crate takes it via `version.workspace = true`, so the binary reports it through CARGO_PKG_VERSION.
# Each `make install` raises the patch number (1.0.x → 1.0.x+1) before building, so an installed
# binary is always distinguishable from the previous one. The bump is left in the working tree,
# not committed: it lands with the next commit.

CARGO   ?= cargo
PREFIX  ?= $(HOME)/.local
BINDIR   = $(PREFIX)/bin
MANIFEST = ymp-rust/Cargo.toml

.PHONY: build bump install uninstall version

version:
	@perl -ne 'if (/^\[workspace\.package\]/){$$w=1} elsif (/^\[/){$$w=0} print "$$1\n" and exit if $$w and /^version\s*=\s*"([^"]+)"/' $(MANIFEST)

bump:
	@perl -0pi -e 's/(\[workspace\.package\][^\[]*?version\s*=\s*")(\d+)\.(\d+)\.(\d+)(")/$$1.$$2.".".$$3.".".($$4+1).$$5/e' $(MANIFEST)
	@echo "version $$($(MAKE) -s version)"

build:
	$(CARGO) build --release --manifest-path $(MANIFEST) --bin ymp

install: bump build
	install -d $(BINDIR)
	install -m 0755 ymp-rust/target/release/ymp $(BINDIR)/ymp
	@echo "installed $(BINDIR)/ymp $$($(MAKE) -s version) — Cargo.toml bumped, commit it with your next change"

uninstall:
	rm -f $(BINDIR)/ymp

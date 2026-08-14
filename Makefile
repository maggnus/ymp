# Build and install the ymp executable.
#
#   make install              build the release binary and install it to ~/.local/bin
#   make install PREFIX=/usr/local
#   make uninstall

CARGO  ?= cargo
PREFIX ?= $(HOME)/.local
BINDIR  = $(PREFIX)/bin

.PHONY: build install uninstall

build:
	$(CARGO) build --release --manifest-path ymp-rust/Cargo.toml --bin ymp

install: build
	install -d $(BINDIR)
	install -m 0755 ymp-rust/target/release/ymp $(BINDIR)/ymp
	@echo "installed $(BINDIR)/ymp"

uninstall:
	rm -f $(BINDIR)/ymp

include env.mk

PROFILE ?= release
TARGET  := target/$(RUST_TARGET)/$(PROFILE)/cps
DESTDIR  ?=

.PHONY: all build install install-man uninstall clean clippy test

all: build

build:
	CARGO_TARGET_DIR=$(CURDIR)/target cargo build --locked --target $(RUST_TARGET) --profile $(PROFILE) --bin cps

install: build install-man
	install -Dm755 $(TARGET) $(DESTDIR)$(PREFIX)/bin/cps
	install -Dm644 themes/cps.py $(DESTDIR)$(PREFIX)/share/cps/themes/cps.py
	install -Dm644 themes/minimal.py $(DESTDIR)$(PREFIX)/share/cps/themes/minimal.py
	install -Dm644 t.desc $(DESTDIR)$(PREFIX)/share/cps/t.desc
	install -Dm644 p.desc $(DESTDIR)$(PREFIX)/share/cps/p.desc
	install -Dm644 cps.toml $(DESTDIR)$(PREFIX)/share/cps/cps.toml

install-man:
	install -d $(DESTDIR)$(PREFIX)/share/man/man1
	install -m 644 docs/cps.1 $(DESTDIR)$(PREFIX)/share/man/man1/

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/cps

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test --locked

clean:
	cargo clean

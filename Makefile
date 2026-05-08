SHELL := /bin/bash

.PHONY: build release test lint fmt clippy doc manpage clean package

build:
	cargo build --locked

release:
	cargo build --locked --release

test:
	cargo test --workspace --locked

lint: fmt clippy

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked

doc:
	cargo doc --workspace --locked

manpage:
	asciidoctor -b manpage -a reproducible --warnings xcat.1.adoc

clean:
	cargo clean

package: manpage release
	@if [[ -z "$(VERSION)" ]]; then \
		echo "VERSION is required, e.g. make package VERSION=2.7.1 TARGET=$$(rustc -vV | sed -n 's/^host: //p')"; \
		exit 1; \
	fi
	@if [[ -z "$(TARGET)" ]]; then \
		echo "TARGET is required, e.g. make package VERSION=2.7.1 TARGET=$$(rustc -vV | sed -n 's/^host: //p')"; \
		exit 1; \
	fi
	./scripts/package "$(VERSION)" "$(TARGET)"

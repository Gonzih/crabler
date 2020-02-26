DATABASE_URL ?= postgres://postgres:password@localhost/vulnerabilities
BACKTRACE ?= 0
CARGO = cargo --color always
CARGO_ARGS = $(if $(RELEASE),--release) $(if $(STATIC_BINARY), --target=x86_64-unknown-linux-musl)

.PHONY: build-nix
build-nix:
	nix-shell shell.nix --run 'make build'

.PHONY: build
build:
	$(CARGO) build $(CARGO_ARGS)

.PHONY: test
test:
	$(CARGO) test $(CARGO_ARGS)

.PHONY: shell
shell:
	nix-shell shell.nix

publish:
	nix-shell shell.nix --run 'cargo publish'

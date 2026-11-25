.PHONY: deps gen check-gen core web build test dev

deps:            ## npm packages for the Vue app
	cd web && npm install

gen:             ## TypeScript of lifeos-wire's types -> web/src/core/generated
	rm -rf web/src/core/generated
	cargo test -q -p lifeos-wire --features ts

check-gen: gen   ## fails when the generated TypeScript is out of date
	git diff --exit-code -- web/src/core/generated

core:            ## the page's wasm link client -> web/src/core/pkg
	crates/lifeos-web-client/bind.sh

web: core        ## the Vue app -> web/dist (embedded by lifeos-web)
	cd web && npm run build

build: web       ## release binary with the web app inside
	cargo build --release -p lifeos

test:            ## Rust, the wasm build and the Vue typecheck
	cargo test -q --workspace
	WASM_PROFILE=dev crates/lifeos-web-client/bind.sh
	cd web && npm run typecheck

dev:             ## backend + Vite with hot reload
	scripts/dev.sh

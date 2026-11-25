.PHONY: deps gen check-gen core web build test dev ios computer

deps:            ## npm packages for the Vue app
	cd web && npm install

gen:             ## TypeScript of lifeos-wire types and tool inputs -> web/src/core/generated
	rm -rf web/src/core/generated
	cargo test -q -p lifeos-wire -p lifeos-core --features ts

check-gen: gen   ## fails when the generated TypeScript is out of date
	git diff --exit-code -- web/src/core/generated

core:            ## the page's wasm link client -> web/src/core/pkg
	crates/lifeos-web-client/bind.sh

web: core        ## the Vue app -> web/dist (embedded by lifeos-web)
	cd web && npm run build

build: web       ## release binary with the web app inside
	cargo build --release -p lifeos -p lifeos-github

computer:        ## LifeOS Computer.app (desktop use), installed in /Applications and served for other Macs
	scripts/build-computer.sh --install
	mkdir -p $(HOME)/.local/share/lifeos && cp "target/release/LifeOS Computer.zip" $(HOME)/.local/share/lifeos/LifeOS-Computer.zip

test:            ## Rust, the wasm build and the Vue typecheck
	cargo test -q --workspace
	WASM_PROFILE=dev crates/lifeos-web-client/bind.sh
	cd web && npm run typecheck

dev:             ## backend + Vite with hot reload
	scripts/dev.sh

ios:             ## build the iPhone app and install it on the plugged-in phone; see docs/ios.md
	scripts/install-ios.sh

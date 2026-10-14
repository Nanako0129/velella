# Run from the repo root: Package.swift links the Rust archive by relative path.
.PHONY: rust bindings build run demo test

rust:
	cargo build --release

# Library mode reads the staticlib itself; no cdylib needed.
bindings: rust
	rm -rf target/bindings && mkdir -p target/bindings
	target/release/uniffi-bindgen generate --library target/release/libvelella_ffi.a \
		--language swift --out-dir target/bindings
	mkdir -p Sources/VelellaFFIC/include Sources/VelellaFFI
	cp target/bindings/velella_ffiFFI.h Sources/VelellaFFIC/include/
	cp target/bindings/velella_ffiFFI.modulemap Sources/VelellaFFIC/include/module.modulemap
	cp target/bindings/velella_ffi.swift Sources/VelellaFFI/

# SwiftPM does not track the Rust archive: drop the executable when it is newer.
# With VELELLA_SIGN_IDENTITY set (a codesign identity name or SHA-1), the executable is re-signed
# under that certificate and a fixed identifier, so its designated requirement stays the same
# across rebuilds; the linker's ad-hoc signature pins it to the binary's hash, which changes on
# every rebuild, and the Keychain asks again each time.
build: rust bindings
	@if [ target/release/libvelella_ffi.a -nt .build/debug/Velella ]; then rm -f .build/debug/Velella; fi
	swift build
	@if [ -n "$$VELELLA_SIGN_IDENTITY" ]; then \
		codesign --force --sign "$$VELELLA_SIGN_IDENTITY" --identifier com.nyanako.velella .build/debug/Velella; fi

run: build
	swift run Velella

# Shell filled with fixture data (no Discord connection).
demo: build
	swift run Velella --demo

test:
	cargo test
	swift test

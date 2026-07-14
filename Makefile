.PHONY: build test run cli mock mock-fresh mock-alias mock-unalias debug-log clean ui-install bundle build-mac build-windows build-linux

RELEASES := releases

build:
	cargo build

test:
	cargo test

ui-install:
	cd app && pnpm install

run: ui-install
	./app/node_modules/.bin/tauri dev

cli:
	cargo run --bin merlinsync-cli

mock-alias:
	sudo ifconfig lo0 alias 192.168.4.1/32 up

mock-unalias:
	-sudo ifconfig lo0 -alias 192.168.4.1

mock: mock-alias
	trap '$(MAKE) mock-unalias' EXIT INT TERM; cargo run --bin merlinsync-mock-device

mock-fresh: mock-alias
	trap '$(MAKE) mock-unalias' EXIT INT TERM; cargo run --bin merlinsync-mock-device -- --fresh

debug-log: ui-install
	RUST_LOG=merlin_protocol=debug ./app/node_modules/.bin/tauri dev

clean:
	cargo clean

build-mac: ui-install
	./app/node_modules/.bin/tauri build
	@mkdir -p $(RELEASES)
	rm -rf "$(RELEASES)/Morgane.app"
	cp -R target/release/bundle/macos/Morgane.app "$(RELEASES)/"
	-cp target/release/bundle/dmg/*.dmg "$(RELEASES)/"
	@echo "→ macOS : $(RELEASES)/Morgane.app (+ .dmg)"

build-windows: ui-install
	./app/node_modules/.bin/tauri build
	@mkdir -p $(RELEASES)
	-cp target/release/bundle/msi/*.msi "$(RELEASES)/"
	-cp target/release/bundle/nsis/*.exe "$(RELEASES)/"
	@echo "→ Windows : installateurs dans $(RELEASES)/"

build-linux: ui-install
	./app/node_modules/.bin/tauri build
	@mkdir -p $(RELEASES)
	-cp target/release/bundle/deb/*.deb "$(RELEASES)/"
	-cp target/release/bundle/appimage/*.AppImage "$(RELEASES)/"
	-cp target/release/bundle/rpm/*.rpm "$(RELEASES)/"
	@echo "→ Linux : paquets dans $(RELEASES)/"

bundle: build-mac


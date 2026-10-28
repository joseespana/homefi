# HomeFi monorepo tasks. Run `make help`.

VERSION := $(shell cargo xtask version 2>/dev/null)
HOST    := $(shell rustc -vV | sed -n 's/^host: //p')
DIST    := dist
ANDROID := apps/android
APK_DIR := $(ANDROID)/app/build/outputs/apk

.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

.PHONY: version
version: ## Print the workspace version
	@echo $(VERSION)

.PHONY: check
check: ## fmt + clippy + tests
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace

.PHONY: test
test: ## Run the Rust tests
	cargo test --workspace

.PHONY: cli
cli: ## Build the CLI (release) for this machine
	cargo build --release -p homefi-cli

.PHONY: install
install: ## Install `homefi` into ~/.cargo/bin
	cargo install --path crates/homefi-cli --locked

.PHONY: android
android: ## Build the release APK (all ABIs)
	cd $(ANDROID) && ./gradlew assembleRelease

.PHONY: android-debug
android-debug: ## Build a debug APK for arm64 only (fast)
	cd $(ANDROID) && ./gradlew assembleDebug -Phomefi.abis=arm64-v8a

.PHONY: android-install
android-install: android-debug ## Install the debug APK on a connected device
	adb install -r $(APK_DIR)/debug/homefi-$(VERSION)-debug.apk

.PHONY: ios
ios: ## Build the iOS core (Rust) and generate the Xcode project, then open it
	apps/ios/scripts/build-core.sh debug
	cd apps/ios && xcodegen
	open apps/ios/HomeFi.xcodeproj

.PHONY: ios-sim
ios-sim: ## Build the iPhone app for the simulator (no signing)
	cd apps/ios && xcodegen -q && xcodebuild -project HomeFi.xcodeproj -scheme HomeFi \
		-destination 'generic/platform=iOS Simulator' -configuration Debug build CODE_SIGNING_ALLOWED=NO -quiet

.PHONY: icons
icons: ## Render assets/brand/*.svg to PNG / ICO
	cargo xtask icons

.PHONY: bump
bump: ## Bump the version: make bump V=1.2.3 (or V=patch|minor|major)
	@test -n "$(V)" || (echo "usage: make bump V=<x.y.z|patch|minor|major>" && exit 1)
	cargo xtask bump $(V)

.PHONY: dist
dist: cli android ## Collect installable builds in dist/
	rm -rf $(DIST) && mkdir -p $(DIST)
	tar -czf $(DIST)/homefi-$(VERSION)-$(HOST).tar.gz -C target/release homefi -C $(CURDIR) README.md LICENSE NOTICE docs/config.example.toml
	/bin/cp $(APK_DIR)/release/homefi-$(VERSION)-release.apk $(DIST)/homefi-$(VERSION)-android.apk
	cd $(DIST) && shasum -a 256 * > SHA256SUMS
	@ls -lh $(DIST)

.PHONY: clean
clean: ## Remove build outputs
	cargo clean
	cd $(ANDROID) && ./gradlew clean
	rm -rf $(DIST) $(ANDROID)/app/src/main/jniLibs

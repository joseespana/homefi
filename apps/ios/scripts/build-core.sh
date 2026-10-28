#!/bin/bash
# Builds homefi-ffi for iPhone and the simulator and packs it for Xcode:
#   apps/ios/Core/HomefiCore.xcframework  (static library + C header + module map)
#   apps/ios/Core/HomefiCore.swift        (UniFFI Swift bindings)
# Both are generated: run this after changing the Rust code (Xcode runs it too).
# Usage: build-core.sh [debug|release]   (release by default)
set -euo pipefail
cd "$(dirname "$0")/../../.."
PROFILE="${1:-release}"
FLAG=$([ "$PROFILE" = release ] && echo --release || echo "")
OUT=apps/ios/Core
GEN=target/ios-bindings
export PATH="$HOME/.cargo/bin:$PATH"
export IPHONEOS_DEPLOYMENT_TARGET=17.0

for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  cargo build -p homefi-ffi --lib $FLAG --target "$target"
done

# Bindings come from the host library (like the Android build).
cargo build -p homefi-ffi --features bindgen
rm -rf "$GEN" && mkdir -p "$GEN"
target/debug/uniffi-bindgen generate --library target/debug/libhomefi_ffi.dylib \
  --language swift --out-dir "$GEN" --no-format
mkdir -p "$GEN/headers"
mv "$GEN/HomefiCoreFFI.h" "$GEN/headers/"
cp "$GEN/HomefiCoreFFI.modulemap" "$GEN/headers/module.modulemap"

rm -rf "$OUT/HomefiCore.xcframework"
mkdir -p "$OUT"
xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/$PROFILE/libhomefi_ffi.a" -headers "$GEN/headers" \
  -library "target/aarch64-apple-ios-sim/$PROFILE/libhomefi_ffi.a" -headers "$GEN/headers" \
  -output "$OUT/HomefiCore.xcframework" >/dev/null
cp "$GEN/HomefiCore.swift" "$OUT/HomefiCore.swift"
echo "HomeFi core for iOS ($PROFILE): $OUT"

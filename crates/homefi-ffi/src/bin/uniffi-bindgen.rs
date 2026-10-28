//! Generates the Kotlin bindings: `cargo run -p homefi-ffi --features bindgen --bin uniffi-bindgen -- generate ...`
fn main() {
    uniffi::uniffi_bindgen_main()
}

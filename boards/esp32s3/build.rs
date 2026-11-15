//! Link with esp-hal's `linkall.x`. If the linker is missing, source `~/export-esp.sh` first.

fn main() {
    println!("cargo:rustc-link-arg=-Tlinkall.x");
}

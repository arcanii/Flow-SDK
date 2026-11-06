//! Names the shared library `@rpath/libflow_sdk_c.dylib` on macOS, as
//! shared libraries there are named, rather than the path cargo built it
//! at: a program linked against it then finds it wherever its rpath says,
//! after the library has been copied elsewhere too.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libflow_sdk_c.dylib");
    }
}

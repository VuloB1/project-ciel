#[cfg(feature = "cpp")]
#[cfg(not(target_os = "macos"))]
fn main() {
    // AIVT patch: dropped `.static_crt(true)` — it forces MSVC's static
    // C++ runtime (/MT), which duplicate-symbol-conflicts (LNK2005,
    // libcpmt.lib vs msvcprt.lib) with the dynamic runtime (/MD) every
    // other native dependency in this workspace already links against
    // (whisper-rs-sys, webrtc_vad). Omitting the call lets `cc` pick the
    // toolchain's default (dynamic on this target), matching everything
    // else. Only affects Windows/Linux; the macOS variant below never used
    // MSVC's CRT distinction in the first place. See
    // crates/providers/embedding/Cargo.toml's `[patch.crates-io]` note.
    cc::Build::new()
        .cpp(true)
        .flag("-std=c++11")
        .file("src/esaxx.cpp")
        .include("src")
        .compile("esaxx");
}

#[cfg(feature = "cpp")]
#[cfg(target_os = "macos")]
fn main() {
    cc::Build::new()
        .cpp(true)
        .flag("-std=c++11")
        .flag("-stdlib=libc++")
        .file("src/esaxx.cpp")
        .include("src")
        .compile("esaxx");
}

#[cfg(not(feature = "cpp"))]
fn main() {}

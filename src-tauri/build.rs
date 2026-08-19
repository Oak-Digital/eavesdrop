fn main() {
    // ScreenCaptureKit's Swift bridge auto-links the concurrency runtime. Point
    // the linker at the SDK stubs so the load command targets macOS' system Swift
    // runtime instead of an older copy from the Xcode toolchain.
    #[cfg(target_os = "macos")]
    if let Ok(output) = std::process::Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        && output.status.success()
    {
        let sdk = String::from_utf8_lossy(&output.stdout);
        let runtime_stubs = std::path::Path::new(sdk.trim()).join("usr/lib/swift");
        if runtime_stubs.exists() {
            println!("cargo:rustc-link-search=native={}", runtime_stubs.display());
            println!("cargo:rustc-link-lib=dylib=swift_Concurrency");
        }
    }

    tauri_build::build()
}

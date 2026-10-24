#[cfg(target_os = "macos")]
fn build_widget_bridge() {
    use std::{env, path::PathBuf, process::Command};

    fn xcrun_output(args: &[&str]) -> String {
        let output = Command::new("xcrun")
            .args(args)
            .output()
            .expect("Xcode tools are required");
        assert!(
            output.status.success(),
            "xcrun failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("xcrun output must be UTF-8")
            .trim()
            .to_owned()
    }

    let source = "widget-bridge/WidgetReloader.swift";
    println!("cargo:rerun-if-changed={source}");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let target = env::var("TARGET").expect("TARGET");
    let arch = match target.as_str() {
        "aarch64-apple-darwin" => "arm64",
        "x86_64-apple-darwin" => "x86_64",
        _ => panic!("unsupported macOS target: {target}"),
    };
    let sdk = xcrun_output(&["--sdk", "macosx", "--show-sdk-path"]);
    let swiftc = PathBuf::from(xcrun_output(&["--find", "swiftc"]));
    let library = out.join("libWidgetReloader.a");
    let status = Command::new(&swiftc)
        .args([
            "-emit-library",
            "-static",
            "-parse-as-library",
            "-swift-version",
            "5",
            "-module-name",
            "GitaViewWidgetBridge",
            "-target",
            &format!("{arch}-apple-macosx11.0"),
            "-sdk",
            &sdk,
            source,
            "-o",
        ])
        .arg(&library)
        .status()
        .expect("failed to run swiftc");
    assert!(
        status.success(),
        "WidgetKit Swift bridge compilation failed"
    );

    // Darwin's linker reads Swift autolink directives from the archive. Supply both
    // the toolchain compatibility libraries and the SDK's system Swift libraries.
    let swift_libs = swiftc
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("lib/swift/macosx");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-search=native={}", swift_libs.display());
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    println!("cargo:rustc-link-lib=static=WidgetReloader");
    println!("cargo:rustc-link-lib=framework=WidgetKit");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}

fn main() {
    #[cfg(target_os = "macos")]
    build_widget_bridge();
    tauri_build::build()
}

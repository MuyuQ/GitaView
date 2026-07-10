fn main() {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let widget_bridge_dir = std::path::Path::new("widget-bridge");
        let obj_path = widget_bridge_dir.join("WidgetReloader.o");
        let lib_path = widget_bridge_dir.join("libWidgetReloader.a");

        // 告诉 Cargo 何时重新编译
        println!(
            "cargo:rerun-if-changed={}",
            widget_bridge_dir.join("WidgetReloader.m").display()
        );

        // 编译 Objective-C 文件
        let status = Command::new("clang")
            .args([
                "-fobjc-arc",
                "-mmacos-version-min=11.0",
                "-c",
                widget_bridge_dir.join("WidgetReloader.m").to_str().unwrap(),
                "-o",
                obj_path.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run clang. Ensure Xcode Command Line Tools are installed.");
        assert!(
            status.success(),
            "WidgetReloader.m compilation failed. Check that clang and Objective-C support are available."
        );

        // 创建静态库
        let status = Command::new("ar")
            .args([
                "rcs",
                lib_path.to_str().unwrap(),
                obj_path.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run ar. Ensure Xcode Command Line Tools are installed.");
        assert!(status.success(), "Static library creation failed.");

        // 告诉 Cargo 链接静态库
        println!("cargo:rustc-link-search=native={}", widget_bridge_dir.display());
        println!("cargo:rustc-link-lib=static=WidgetReloader");
    }

    tauri_build::build()
}

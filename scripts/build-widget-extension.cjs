const { execSync } = require("child_process");
const path = require("path");
const fs = require("fs");

if (process.platform !== "darwin") {
  console.log("Skipping Widget Extension build (not macOS)");
  process.exit(0);
}

// 检查是否有完整的 Xcode（不只是 Command Line Tools）
try {
  const xcodePath = execSync("xcode-select -p", { encoding: "utf-8" }).trim();
  if (!xcodePath.includes("Xcode.app")) {
    console.log("Skipping Widget Extension build (full Xcode not found, using Command Line Tools)");
    process.exit(0);
  }
} catch (e) {
  console.log("Skipping Widget Extension build (xcode-select failed)");
  process.exit(0);
}

const extDir = path.join(__dirname, "..", "src-tauri", "widget-extension");

try {
  console.log("Generating Xcode project...");
  execSync("xcodegen generate", { cwd: extDir, stdio: "inherit" });

  console.log("Building Widget Extension...");
  execSync(
    "xcodebuild -project GitaViewWidget.xcodeproj -scheme GitaViewWidgetExtension -configuration Release -derivedDataPath build",
    { cwd: extDir, stdio: "inherit" }
  );
  console.log("Widget Extension build complete");
} catch (e) {
  console.error("Widget Extension build failed:", e.message);
  process.exit(1);
}

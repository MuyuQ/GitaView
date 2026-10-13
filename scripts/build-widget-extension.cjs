const { execFileSync } = require("child_process");
const path = require("path");

// tauri.conf.json 的 bundle.macOS.files 无条件映射 .appex：
// 打包（tauri build）阶段缺 .appex 必然失败，因此"即将打包却没构建扩展"要 fail hard。
// 判据：tauri CLI 执行 beforeBundleCommand 时会注入 TAURI_ENV_* 环境变量；
// CI validate 等独立调用（不随后打包）保持软跳过 + 显式警告。
// 特殊情况可用 GITAVIEW_ALLOW_SKIP_WIDGET_EXTENSION=1 恢复软跳过（打包仍会失败）。
const allowSkip = process.env.GITAVIEW_ALLOW_SKIP_WIDGET_EXTENSION === "1";
const isBundleContext = Boolean(process.env.TAURI_ENV_PLATFORM);

function skip(message) {
  if (process.platform === "darwin" && !allowSkip && isBundleContext) {
    console.error(`ERROR: ${message}`);
    console.error(
      "macOS 打包需要 Widget Extension（bundle.macOS.files 已映射 .appex）。\n" +
        "请安装完整版 Xcode 并运行 `xcode-select -s /Applications/Xcode.app`。",
    );
    process.exit(1);
  }
  if (process.platform === "darwin" && !allowSkip) {
    console.warn(
      `WARNING: skipping Widget Extension build (${message})；` +
        "本次调用不会打包，但请勿在 tauri build 的 beforeBundleCommand 场景依赖此跳过",
    );
  }
  console.log(`Skipping Widget Extension build (${message})`);
  process.exit(0);
}

if (process.platform !== "darwin") {
  skip("not macOS");
}

// 检查是否有完整的 Xcode（不只是 Command Line Tools）
try {
  const xcodePath = execFileSync("xcode-select", ["-p"], { encoding: "utf-8" }).trim();
  if (!xcodePath.includes("Xcode.app")) {
    skip("full Xcode not found, using Command Line Tools");
  }
} catch {
  skip("xcode-select failed");
}

const extDir = path.join(__dirname, "..", "src-tauri", "widget-extension");

// 正式发布时由 CI 注入签名身份（release.yml 的 tauri-action 步骤会导出这些变量），
// 本地/无签名 CI 保持 project.yml 的 ad-hoc 配置。
// hardened runtime 必须开启：公证与 pluginkit 加载扩展都要求它。
function signingOverrides() {
  const overrides = [];
  if (process.env.APPLE_SIGNING_IDENTITY) {
    overrides.push(`CODE_SIGN_IDENTITY=${process.env.APPLE_SIGNING_IDENTITY}`);
    overrides.push("ENABLE_HARDENED_RUNTIME=YES");
    if (process.env.APPLE_TEAM_ID) {
      overrides.push(`DEVELOPMENT_TEAM=${process.env.APPLE_TEAM_ID}`);
    }
  }
  return overrides;
}

function xcodebuild(args) {
  execFileSync("xcodebuild", args, { cwd: extDir, stdio: "inherit" });
}

try {
  console.log("Generating Xcode project...");
  execFileSync("xcodegen", ["generate"], { cwd: extDir, stdio: "inherit" });

  console.log("Building Widget Extension...");
  xcodebuild([
    "-project",
    "GitaViewWidget.xcodeproj",
    "-scheme",
    "GitaViewWidgetExtension",
    "-configuration",
    "Release",
    "-derivedDataPath",
    "build",
    ...signingOverrides(),
  ]);
  console.log("Widget Extension build complete");

  // 跨语言数据契约测试（与 Rust widget_payload_matches_cross_language_fixture 共享 fixture）
  console.log("Running Widget data contract tests...");
  xcodebuild([
    "test",
    "-project",
    "GitaViewWidget.xcodeproj",
    "-scheme",
    "GitaViewWidgetTests",
    "-destination",
    "platform=macOS",
  ]);
  console.log("Widget data contract tests passed");
} catch (e) {
  console.error("Widget Extension build/test failed:", e.message);
  process.exit(1);
}

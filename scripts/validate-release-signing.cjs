const platform = process.env.RELEASE_PLATFORM ?? "";

const requiredByPlatform = platform.startsWith("windows")
  ? [
      "WINDOWS_CERTIFICATE",
      "WINDOWS_CERTIFICATE_PASSWORD",
      "WINDOWS_CERTIFICATE_THUMBPRINT",
      "WINDOWS_TIMESTAMP_URL",
    ]
  : platform.startsWith("macos")
    ? [
        "APPLE_CERTIFICATE",
        "APPLE_CERTIFICATE_PASSWORD",
        "APPLE_SIGNING_IDENTITY",
        "APPLE_ID",
        "APPLE_PASSWORD",
        "APPLE_TEAM_ID",
      ]
    : [];

if (requiredByPlatform.length === 0) {
  console.error(`Unsupported release platform: ${platform || "<empty>"}`);
  process.exit(1);
}

const missing = requiredByPlatform.filter((name) => !process.env[name]?.trim());
if (missing.length > 0) {
  console.error(`Missing release signing secrets for ${platform}: ${missing.join(", ")}`);
  process.exit(1);
}

console.log(`Validated release signing secrets for ${platform}.`);

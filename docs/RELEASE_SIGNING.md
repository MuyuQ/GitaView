# Release Signing

GitaView requires platform signing credentials for ordinary version tags such as `v0.3.0`. Do not publish ordinary release artifacts generated without these secrets.

## Unsigned Draft Prerelease Channel

Early test builds may use a tag ending in `-unsigned`, such as `v0.3.0-unsigned`. This explicit suffix is the only way to bypass signing preflight. The workflow skips Windows certificate import and signature verification, builds without macOS signing credentials, marks the GitHub release as a draft prerelease, and adds an unsigned-artifact warning to the release notes.

Unsigned artifacts are for testing only:

- Windows SmartScreen may warn or block installation.
- macOS Gatekeeper may warn or block installation.
- Do not present unsigned artifacts as stable releases or signed release candidates.

## Windows Secrets

- `WINDOWS_CERTIFICATE`: base64-encoded `.pfx` code-signing certificate.
- `WINDOWS_CERTIFICATE_PASSWORD`: `.pfx` password.
- `WINDOWS_CERTIFICATE_THUMBPRINT`: expected certificate thumbprint.
- `WINDOWS_TIMESTAMP_URL`: RFC 3161 timestamp server URL supplied by the certificate provider.

The release workflow imports the certificate into `Cert:\CurrentUser\My`, patches the runner-local Tauri configuration, builds the installers, and verifies Authenticode signatures.

## macOS Secrets

- `APPLE_CERTIFICATE`: base64-encoded Developer ID Application certificate export.
- `APPLE_CERTIFICATE_PASSWORD`: certificate export password.
- `APPLE_SIGNING_IDENTITY`: Developer ID Application identity.
- `APPLE_ID`: Apple account used for notarization.
- `APPLE_PASSWORD`: app-specific password.
- `APPLE_TEAM_ID`: Apple Developer team ID.

Tauri receives these values during the macOS release jobs and performs signing and notarization.

## Stale Draft Release Cleanup

The historical draft `v0.2.2` contains `0.1.0` binaries and must not be published. Inspect it before deletion:

```bash
gh release view v0.2.2
```

After confirming the stale draft can be discarded, delete it and its tag explicitly:

```bash
gh release delete v0.2.2 --cleanup-tag
```

Create a fresh version commit and either a signed ordinary tag or an explicitly unsigned testing tag.

## Real-Device Smoke Checks

Before publishing a signed draft:

- Verify the Windows installer has a valid Authenticode signature and opens without console-window flashes during refresh.
- Verify the Windows widget remains anchored after resizing, multi-monitor movement, and Explorer restart.
- Verify each macOS DMG installs on its target architecture and passes Gatekeeper checks.
- Verify the macOS menu-bar icon adapts to light and dark menu bars, opens on left click, and does not appear in Dock or Cmd+Tab.

For an unsigned draft prerelease, perform the widget behavior checks on each target platform and keep the draft unpublished. Signing and Gatekeeper checks remain required before creating a stable release candidate.

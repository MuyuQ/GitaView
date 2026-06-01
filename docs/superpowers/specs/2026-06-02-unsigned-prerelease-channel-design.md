# Unsigned Prerelease Channel Design

## Goal

Publish `v0.3.0-unsigned` as an explicitly unsigned draft prerelease without
weakening the signed release path used by normal version tags such as
`v0.3.0`.

## Version And Tag Contract

The application metadata advances from `0.2.2` to `0.3.0` in `package.json`,
`package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and
`src-tauri/tauri.conf.json`.

The release version validator accepts exactly two tag shapes for version
`0.3.0`:

- `v0.3.0` for a normal signed release.
- `v0.3.0-unsigned` for an unsigned test prerelease.

No environment variable or repository variable can disable signing for an
ordinary tag. The `-unsigned` suffix is the only opt-in mechanism.

## Workflow Behavior

The release workflow derives whether a build is unsigned from the tag suffix
and exposes that result to later steps.

For ordinary tags, the existing signing behavior remains mandatory:

- Validate Windows or macOS signing secrets.
- Import the Windows certificate before packaging.
- Pass macOS signing and notarization secrets to Tauri.
- Verify Windows installer Authenticode signatures after packaging.

For a tag ending in `-unsigned`, the workflow skips signing preflight,
Windows certificate import, and Windows signature verification. Tauri builds
the Windows and macOS artifacts without signing credentials.

All releases remain drafts. Unsigned releases are additionally marked as
GitHub prereleases and their release notes state that the artifacts are
unsigned test builds. The notes warn that Windows SmartScreen and macOS
Gatekeeper may block installation and that the artifacts must not be
presented as stable releases.

## Documentation

`docs/RELEASE_SIGNING.md` documents both channels:

- Normal tags require configured platform credentials.
- Tags ending in `-unsigned` create unsigned draft prereleases for testing
  only.
- Unsigned packages require additional local caution and are not substitutes
  for signed release candidates.

## Testing

Source-contract tests lock the workflow boundary:

- The validator recognizes the `-unsigned` suffix.
- The workflow derives unsigned mode from the tag suffix.
- Signing-only steps are conditional on unsigned mode being false.
- The GitHub prerelease flag follows unsigned mode.
- Release notes identify unsigned test artifacts.
- Existing signed-release assertions remain in place.

Command-level checks exercise the release version validator with
`v0.3.0`, `v0.3.0-unsigned`, and an invalid suffix. The final verification also
runs frontend tests, Rust tests, formatting, Clippy, the frontend build, and a
local Tauri debug build without bundling.


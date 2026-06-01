# Unsigned Prerelease Channel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish `v0.3.0-unsigned` as an explicitly unsigned draft prerelease while preserving mandatory signing for ordinary release tags.

**Architecture:** Keep tag interpretation in `scripts/validate-release-version.cjs`, which validates the current package version and writes an `unsigned` GitHub Actions output. The release workflow consumes that output to skip signing-only steps for the explicit `-unsigned` channel and to mark those artifacts as draft prerelease test builds. Ordinary tags continue through the existing signing gate unchanged.

**Tech Stack:** GitHub Actions YAML, Node.js CommonJS release validation, Vitest source-contract tests, npm metadata, Cargo metadata, Tauri 2 configuration.

---

### Task 1: Lock The Unsigned Channel Contract

**Files:**
- Modify: `src/lib/crossPlatformHardeningContract.test.ts`
- Modify: `src/lib/reviewRemediationContract.test.ts`

- [ ] **Step 1: Add failing source-contract assertions**

Extend the release trust contract so it expects `id: release_mode`, the exact
`-unsigned` suffix, conditional signing steps, prerelease output wiring, and an
unsigned warning. Update the version assertions from `0.2.2` to `0.3.0`.

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```bash
npm test -- src/lib/crossPlatformHardeningContract.test.ts src/lib/reviewRemediationContract.test.ts
```

Expected: FAIL because the workflow does not expose `release_mode`, the
validator does not recognize `-unsigned`, and metadata is still `0.2.2`.

### Task 2: Implement Explicit Unsigned Release Mode

**Files:**
- Modify: `scripts/validate-release-version.cjs`
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Add validator output for the explicit suffix**

Update the validator to accept only the normal tag and its `-unsigned`
variant, then append the GitHub Actions output:

```js
const expectedTag = `v${packageJson.version}`;
const unsignedTag = `${expectedTag}-unsigned`;
const releaseTag = process.env.GITHUB_REF_NAME;

if (![expectedTag, unsignedTag].includes(releaseTag)) {
  throw new Error(`Release tag must be ${expectedTag} or ${unsignedTag}`);
}

const unsigned = releaseTag === unsignedTag;
if (process.env.GITHUB_OUTPUT) {
  fs.appendFileSync(process.env.GITHUB_OUTPUT, `unsigned=${unsigned}\n`);
}
```

- [ ] **Step 2: Make signing-only workflow steps conditional**

Assign `id: release_mode` to the version validation step. Add:

```yaml
if: steps.release_mode.outputs.unsigned != 'true'
```

to signing-secret validation, and combine it with the existing Windows
platform condition for Windows certificate import and signature verification:

```yaml
if: startsWith(matrix.platform, 'windows') && steps.release_mode.outputs.unsigned != 'true'
```

Set the Tauri action prerelease flag from the validated output:

```yaml
prerelease: ${{ steps.release_mode.outputs.unsigned == 'true' }}
```

Add a release-note line that resolves to an unsigned test-build warning for
the unsigned channel and a signed-release description for normal tags.

- [ ] **Step 3: Run focused tests**

Run:

```bash
npm test -- src/lib/crossPlatformHardeningContract.test.ts src/lib/reviewRemediationContract.test.ts
```

Expected: the unsigned-channel assertions pass except for the metadata
assertions, which remain RED until Task 3.

### Task 3: Advance Application Metadata And Documentation

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `docs/RELEASE_SIGNING.md`

- [ ] **Step 1: Advance application metadata**

Update the GitaView package version from `0.2.2` to `0.3.0` in npm metadata,
the Tauri configuration, `src-tauri/Cargo.toml`, and only the `gitaview`
package entry in `src-tauri/Cargo.lock`. Do not change the unrelated
`alloc-stdlib` dependency version.

- [ ] **Step 2: Document both release paths**

Document that ordinary version tags remain signed-only and tags ending in
`-unsigned` create draft prereleases for testing. Add warnings that Windows
SmartScreen and macOS Gatekeeper may block unsigned packages and that the
unsigned channel is not a stable-release substitute.

- [ ] **Step 3: Run focused tests to verify GREEN**

Run:

```bash
npm test -- src/lib/crossPlatformHardeningContract.test.ts src/lib/reviewRemediationContract.test.ts
```

Expected: PASS.

### Task 4: Validate The Release Boundary

**Files:**
- Verify: `scripts/validate-release-version.cjs`
- Verify: `.github/workflows/release.yml`

- [ ] **Step 1: Validate the normal signed tag**

Run:

```powershell
$env:GITHUB_REF_NAME='v0.3.0'; node scripts/validate-release-version.cjs
```

Expected: exit `0` and `Validated signed release version 0.3.0`.

- [ ] **Step 2: Validate the unsigned prerelease tag and output**

Run:

```powershell
$outputFile = Join-Path $env:TEMP 'gitaview-release-output.txt'
Remove-Item -LiteralPath $outputFile -ErrorAction SilentlyContinue
$env:GITHUB_REF_NAME='v0.3.0-unsigned'
$env:GITHUB_OUTPUT=$outputFile
node scripts/validate-release-version.cjs
Get-Content -LiteralPath $outputFile
```

Expected: exit `0`, `Validated unsigned release version 0.3.0`, and
`unsigned=true`.

- [ ] **Step 3: Reject an unrecognized suffix**

Run:

```powershell
$env:GITHUB_REF_NAME='v0.3.0-beta'; Remove-Item Env:GITHUB_OUTPUT -ErrorAction SilentlyContinue; node scripts/validate-release-version.cjs
```

Expected: non-zero exit and `Release tag must be v0.3.0 or v0.3.0-unsigned`.

### Task 5: Run Full Verification And Publish Through PR

**Files:**
- Verify all modified files

- [ ] **Step 1: Run complete verification**

Run:

```bash
npm test
npm audit --audit-level=low
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri -- build --debug --no-bundle
git diff --check
```

Expected: every command exits `0`.

- [ ] **Step 2: Commit the implementation**

Stage only the implementation files and commit:

```bash
git add .github/workflows/release.yml docs/RELEASE_SIGNING.md package.json package-lock.json scripts/validate-release-version.cjs src/lib/crossPlatformHardeningContract.test.ts src/lib/reviewRemediationContract.test.ts src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "ci: add unsigned prerelease channel"
```

- [ ] **Step 3: Push branch and open a pull request**

Run:

```bash
git push -u origin codex/release-v0.3.0-unsigned
gh pr create --base main --head codex/release-v0.3.0-unsigned --title "[codex] add unsigned v0.3.0 prerelease channel" --body-file <temporary-markdown-file>
```

Expected: branch push succeeds and GitHub returns a PR URL.

- [ ] **Step 4: Merge after CI and push the release tag**

After required PR checks pass, merge the PR, fetch `origin/main`, create the
annotated tag at the merged `origin/main`, and push it:

```bash
git fetch origin main
git tag -a v0.3.0-unsigned origin/main -m "GitaView v0.3.0 unsigned test prerelease"
git push origin v0.3.0-unsigned
```

Expected: the tag push triggers the unsigned draft prerelease workflow.


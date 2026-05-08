# Releasing

## First-Time Setup

These steps are required once before the very first release.

### 1. Create package manager repos

The package manager repos are maintained separately:

- **Homebrew cask**: [`longvh/homebrew-dome`](https://github.com/longvh/homebrew-dome)
- **Scoop bucket**: [`longvh/scoop-dome`](https://github.com/longvh/scoop-dome)

Each repo contains a GitHub Actions workflow that receives dispatch events from the main Dome repo and auto-updates the package manifest.

### 2. Create a GitHub PAT

Create a [GitHub PAT (classic)](https://github.com/settings/tokens) with `repo` scope. Add it as a repository secret named `RELEASE_DISPATCH_TOKEN` in the main Dome repo. This token is used by the dispatch workflow to trigger updates in the Homebrew and Scoop repos.

### 3. Replace WiX placeholder GUIDs

`resources/windows/main.wxs` ships with a placeholder `UpgradeCode` GUID. Before the first release, replace it with a real GUID:

```powershell
[guid]::NewGuid()
```

The `UpgradeCode` GUID must **never change** after the first release — Windows uses it to identify the product across upgrades.

## Release Process

1. Update `version` in `Cargo.toml`
2. Commit the version bump
3. Push a version tag:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
4. The release workflow (`.github/workflows/release.yml`) validates the tag matches `Cargo.toml`, builds all platform artifacts, and creates a **draft** GitHub Release
5. Review the draft in the GitHub UI — verify artifacts are attached and release notes are correct
6. **Publish** the draft release — this triggers the dispatch workflow (`.github/workflows/dispatch.yml`) which auto-updates the Homebrew cask and Scoop bucket
7. Verify package managers:
   ```bash
   # macOS
   brew tap longvh/dome && brew install --cask dome

   # Windows
   scoop bucket add dome https://github.com/longvh/scoop-dome
   scoop install dome
   ```

## CI Artifacts

The release workflow produces:

| Platform | Artifacts |
|----------|-----------|
| macOS (aarch64, x86_64) | `.dmg` (with `Dome.app` bundle), `.tar.gz` (binary) |
| Windows (x86_64) | `.msi` installer (via WiX), `.zip` (portable, used by Scoop) |
| All | `checksums-sha256.txt` |

`resources/windows/Dome.ico` and `resources/windows/dome.manifest` are compiled into `dome.exe` via `build.rs` and are not packaged separately by the WiX installer.

## How It Works

Two workflows coordinate the release:

- **release.yml** — triggered by `v*.*.*` tag push. Validates the tag version matches `Cargo.toml`, builds macOS (two architectures) and Windows artifacts, then creates a draft GitHub Release with all artifacts and auto-generated release notes.
- **dispatch.yml** — triggered when a release is published. Downloads the relevant artifacts, computes SHA256 hashes, and sends `repository_dispatch` events to `longvh/homebrew-dome` and `longvh/scoop-dome` with the version and checksums. Each package repo has its own workflow that updates the manifest and commits.

The dispatch steps use `continue-on-error: true` so a failure in one package manager doesn't block the other.

# Releasing

## Release process

1. Update `version` in `Cargo.toml`
2. Commit the version bump
3. Push a version tag:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
4. The release workflow (`.github/workflows/release.yml`) validates the tag matches `Cargo.toml`, builds all platform artifacts, and creates a **draft** GitHub Release
5. Review the draft in the GitHub UI. Verify artifacts are attached and release notes are correct
6. **Publish** the draft release. This triggers the dispatch workflow (`.github/workflows/dispatch.yml`) which auto-updates the Homebrew cask and Scoop bucket
7. Verify package managers:
   ```bash
   # macOS
   brew tap l0ngvh/dome && brew install --cask dome

   # Windows
   scoop bucket add dome https://github.com/l0ngvh/scoop-dome
   scoop install dome
   ```

## CI artifacts

The release workflow produces:

| Platform | Artifacts |
|----------|-----------|
| macOS (aarch64, x86_64) | `.dmg` (with `Dome.app` bundle), `.tar.gz` (binary) |
| Windows (x86_64) | `.msi` installer (via WiX), `.zip` (portable, used by Scoop) |
| All | `checksums-sha256.txt` |

`resources/windows/Dome.ico` and `resources/windows/dome.manifest` are compiled into `dome.exe` via `build.rs` and are not packaged separately by the WiX installer.

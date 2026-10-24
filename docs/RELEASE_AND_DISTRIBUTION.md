# Packaging, Distribution & Release Guide

This document describes the Continuous Integration, Continuous Delivery (CI/CD), packaging architecture, and multi-OS distribution pipelines for Open Market Terminal (`omt`).

---

## 1. Supported Operating Systems & Architectures

OMT publishes pre-compiled, optimized standalone packages for all major operating systems and CPU architectures:

| Platform / Operating System | Architecture | Target Triple | Distribution Format | Static / C Runtime |
|---|---|---|---|---|
| **Linux (Standard / Distros)** | x86_64 (64-bit AMD/Intel) | `x86_64-unknown-linux-gnu` | `.tar.gz`, `.deb` | Glibc (Ubuntu, Debian, Fedora, Arch) |
| **Linux (Alpine / Containers)** | x86_64 (64-bit AMD/Intel) | `x86_64-unknown-linux-musl` | `.tar.gz` | Musl (100% static, zero glibc dependency) |
| **Linux (ARM Servers / Pi 4/5)** | aarch64 (64-bit ARM) | `aarch64-unknown-linux-gnu` | `.tar.gz`, `.deb` | Glibc (Graviton, Raspberry Pi OS 64-bit) |
| **Linux (ARM Musl)** | aarch64 (64-bit ARM) | `aarch64-unknown-linux-musl` | `.tar.gz` | Musl (100% static ARM binary) |
| **macOS (Apple Silicon)** | aarch64 (Apple Silicon M1-M4) | `aarch64-apple-darwin` | `.tar.gz` | Native Darwin Mach-O |
| **macOS (Intel)** | x86_64 (Legacy Intel Macs) | `x86_64-apple-darwin` | `.tar.gz` | Native Darwin Mach-O |
| **macOS (Universal 2)** | Universal (ARM64 + x86_64) | `universal-apple-darwin` | `.tar.gz` | Multi-arch fat binary (`lipo`) |
| **Windows** | x86_64 (64-bit Windows) | `x86_64-pc-windows-msvc` | `.zip` | Native MSVC (`omt.exe`) |
| **Windows (ARM64)** | aarch64 (Surface / Snapdragon) | `aarch64-pc-windows-msvc` | `.zip` | Native MSVC ARM64 (`omt.exe`) |

---

## 2. Package Contents

Every distributed release archive (`.tar.gz` or `.zip`) contains a self-contained distribution bundle:

- `omt` / `omt.exe` — Stripped release binary compiled with Link-Time Optimization (`lto = "thin"`).
- `README.md` — Complete documentation, keybindings, and architectural overview.
- `LICENSE` — Full MIT license.
- `THIRD_PARTY_NOTICES.md` — Complete inventory of third-party dependencies and their licenses.
- `examples/config.toml` — Sample configuration file with environment credential guides.

Debian packages (`.deb`) install:
- `/usr/bin/omt`
- `/usr/share/doc/omt/README.md`
- `/usr/share/doc/omt/copyright`
- `/usr/share/doc/omt/examples/config.toml`

---

## 3. Installation Methods

### 3.1 One-Line Installer (Linux & macOS)

To install or update to the latest release automatically with architecture detection and SHA-256 integrity verification:

```bash
curl -proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/MuhammedZohaib/open-market-terminal/main/scripts/install.sh | bash
```

Custom installation directory:
```bash
INSTALL_DIR=~/.local/bin curl -proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/MuhammedZohaib/open-market-terminal/main/scripts/install.sh | bash
```

### 3.2 One-Line Installer (Windows PowerShell)

Run in PowerShell:

```powershell
irm https://raw.githubusercontent.com/MuhammedZohaib/open-market-terminal/main/scripts/install.ps1 | iex
```

The script downloads the matching archive, verifies checksums, extracts `omt.exe` into `%LOCALAPPDATA%\Programs\omt\bin`, and registers the directory in the user's `PATH`.

### 3.3 Debian / Ubuntu (`.deb`)

Download the appropriate `.deb` package from the GitHub Releases page:

```bash
# For x86_64
sudo dpkg -i omt_<version>_amd64.deb

# For ARM64
sudo dpkg -i omt_<version>_arm64.deb
```

### 3.4 Homebrew (macOS & Linux)

```bash
brew tap MuhammedZohaib/open-market-terminal https://github.com/MuhammedZohaib/open-market-terminal
brew install omt
```

### 3.5 Build from Source via Cargo

```bash
cargo install --locked --path crates/omt-app
```

---

## 4. Checksum Verification

Every release includes individual `.sha256` files and a unified `SHA256SUMS.txt` catalog.

To verify a downloaded release archive:

### Linux / macOS
```bash
sha256sum -c omt-v0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
```

### Windows PowerShell
```powershell
(Get-FileHash -Algorithm SHA256 .\omt-v0.1.0-x86_64-pc-windows-msvc.zip).Hash
```

---

## 5. CI/CD Architecture

### 5.1 Continuous Integration (`.github/workflows/ci.yml`)

Runs on every push to `main` and on pull requests:

- **Lint & Documentation Job**:
  - `cargo fmt --all -- --check`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo doc --locked --workspace --no-deps`
- **Test Matrix Job**:
  - Concurrently tests across `ubuntu-latest`, `macos-latest`, and `windows-latest`.
  - Runs full workspace test suites (`cargo test --locked --workspace`).
  - Executes release build and headless terminal snapshot smoke test (`cargo run --locked --release -- --offline --snapshot`).
- **Packaging Dry-Run Job**:
  - Exercises `scripts/package.sh` (with `.deb` generation) and `scripts/package.ps1` in CI.
  - Guarantees packaging scripts, tarballs, zip archives, and checksum generators never break unexpectedly.

### 5.2 Release Automation (`.github/workflows/release.yml`)

Triggered on tag pushes (`v*`) or manual `workflow_dispatch`:

1. **`build-linux`**:
   - Cross-compiles for `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu`, and `aarch64-unknown-linux-musl`.
   - Generates `.deb` packages for Debian/Ubuntu x86_64 and ARM64.
   - Computes SHA256 checksums.
2. **`build-macos`**:
   - Compiles native Apple Silicon (`aarch64-apple-darwin`) and Intel (`x86_64-apple-darwin`).
   - Merges both into a Universal 2 fat binary using `lipo`.
   - Packages all three variants with SHA256 checksums.
3. **`build-windows`**:
   - Compiles native Windows x86_64 and Windows ARM64.
   - Packages `.zip` archives with metadata and SHA256 checksums.
4. **`publish`**:
   - Downloads all artifacts from runner jobs.
   - Generates unified `SHA256SUMS.txt`.
   - Injects computed checksums into `Formula/omt.rb`.
   - Publishes GitHub Release with automatically generated release notes.
   - Attaches all release assets, checksum files, and package manager formulas.
   - Outputs a formatted release summary table to the GitHub Actions job summary.

---

## 6. How to Cut a New Release

1. Update version in `Cargo.toml` (`[workspace.package]` version).
2. Commit and push the version update:
   ```bash
   git commit -am "chore: bump version to v0.2.0"
   git push origin main
   ```
3. Tag the commit with semantic versioning:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
4. GitHub Actions automatically executes `.github/workflows/release.yml`, compiles binaries across all 9 target triples, packages archives, generates checksums, and publishes the GitHub Release.

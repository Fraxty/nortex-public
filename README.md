# NORTEX

**NORTEX Code — Public Preview 0.1.0**

NORTEX is an independent programming-language and IDE project. This repository distributes the
Windows application and its complete Rust source distribution. The inspectable source tree is in
[`source/`](source/); a verified source archive is also available below.

## Downloads

| File | Description |
|---|---|
| [NORTEX-Setup-x64.exe](https://github.com/Fraxty/nortex-public/raw/refs/heads/main/NORTEX-Setup-x64.exe) | Per-user Windows installer |
| [NORTEX-Portable-x64.zip](https://github.com/Fraxty/nortex-public/raw/refs/heads/main/NORTEX-Portable-x64.zip) | Portable package; run `bin\nortex-code.exe` |
| [nortex-0.1.0-src.zip](https://github.com/Fraxty/nortex-public/raw/refs/heads/main/nortex-0.1.0-src.zip) | Complete, checksummed NORTEX source distribution |
| [SHA256SUMS.txt](https://github.com/Fraxty/nortex-public/raw/refs/heads/main/SHA256SUMS.txt) | SHA-256 checksums |

**Platform:** Windows 11 x64 (tested). The application manifest declares Windows 10/11 compatibility,
but Windows 10 22H2 runtime and installer tests remain outstanding because no Windows 10 test
machine/VM is available. The binaries are not digitally signed; SmartScreen may show a warning.
Verify downloads against the checksums. See [INSTALLATION.md](INSTALLATION.md).

## Included project components

- NORTEX language, compiler, runtime, and NORTEX Code IDE
- Extension host and an offline catalog containing 8 bundled and 17 optional extension packages
- GitHub Issues for feedback and bug reports; report security issues privately as described in
  [SECURITY.md](SECURITY.md)

The extension catalog is local and offline; it is not a live online marketplace. Package
capabilities vary, and the preview has known language and integration limitations. See
[RELEASE_NOTES.md](RELEASE_NOTES.md) before relying on a feature.

## Source and license

The source distribution includes the compiler, runtime, IDE, extensions, tools, examples, tests,
build scripts and public documentation. Follow [`source/README.md`](source/README.md) to build it.
The project is provided under the MIT License in [LICENSE](LICENSE). Third-party notices
are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

NORTEX is under active development. Features may change, and this preview is not intended for
production use. See [PRIVACY.md](PRIVACY.md) for the documented data-collection behavior.

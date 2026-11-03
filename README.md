# NORTEX

**NORTEX Code — Public Preview 0.1.0**

NORTEX is an independent programming-language and IDE project. This repository distributes the
Windows application; it does not contain the complete source code. The preview is therefore not
a request for source-level review.

## Downloads

| File | Description |
|---|---|
| [NORTEX-Setup-x64.exe](https://github.com/Fraxty/nortex-public/raw/refs/heads/main/NORTEX-Setup-x64.exe) | Per-user Windows installer |
| [NORTEX-Portable-x64.zip](https://github.com/Fraxty/nortex-public/raw/refs/heads/main/NORTEX-Portable-x64.zip) | Portable package; run `bin\nortex-code.exe` |
| [SHA256SUMS.txt](https://github.com/Fraxty/nortex-public/raw/refs/heads/main/SHA256SUMS.txt) | SHA-256 checksums |

**Platform:** Windows 11 x64. Windows 10 is untested and unsupported. The binaries are not
digitally signed; SmartScreen may show a warning. Verify downloaded files against the checksums.
See [INSTALLATION.md](INSTALLATION.md).

## Included project components

- NORTEX language, compiler, runtime, and NORTEX Code IDE
- Extension host and an offline catalog containing 8 bundled and 17 optional extension packages
- GitHub Issues for feedback and bug reports; report security issues privately as described in
  [SECURITY.md](SECURITY.md)

The extension catalog is local and offline; it is not a live online marketplace. Package
capabilities vary, and the preview has known language and integration limitations. See
[RELEASE_NOTES.md](RELEASE_NOTES.md) before relying on a feature.

## Source and license

The downloadable preview is closed-source in this repository; no source-level code review is
possible from these files. The distributed software is provided under the MIT License in
[LICENSE.txt](LICENSE.txt). Third-party notices are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

NORTEX is under active development. Features may change, and this preview is not intended for
production use. See [PRIVACY.md](PRIVACY.md) for the documented data-collection behavior.

# ssimulacra2_rs

[![Crates.io](https://img.shields.io/crates/v/ssimulacra2_rs?style=for-the-badge)](https://crates.io/crates/ssimulacra2_rs)
[![LICENSE](https://img.shields.io/crates/l/ssimulacra2_rs?style=for-the-badge)](https://github.com/rust-av/ssimulacra2_bin/blob/main/LICENSE)

Binary interface to the Rust implementation of the SSIMULACRA2 metric: https://github.com/rust-av/ssimulacra2

## Required packages for video support:

### Arch

```bash
sudo pacman -S vapoursynth vapoursynth-plugin-lsmashsource gcc make cmake pkg-config ttf-bitstream-vera # Keep install dependencies
```

### Other Linux

See http://www.vapoursynth.com/doc/installation.html#linux-installation

TODO: How to install LSMASHSource?

### Windows

See http://www.vapoursynth.com/doc/installation.html#windows-installation

Then download the latest release-x86_64-cachedir-cwd.zip from https://github.com/AkarinVS/L-SMASH-Works/releases/tag/vA.3j

# ssimulacra2_rs

[![Crates.io](https://img.shields.io/crates/v/ssimulacra2_rs?style=for-the-badge)](https://crates.io/crates/ssimulacra2_rs)
[![LICENSE](https://img.shields.io/crates/l/ssimulacra2_rs?style=for-the-badge)](https://github.com/rust-av/ssimulacra2_bin/blob/main/LICENSE)

Binary interface to the Rust implementation of the SSIMULACRA2 metric: https://github.com/rust-av/ssimulacra2

## Quality Guidelines

The following is a rough estimate of how ssimulacra2 scores correspond to visual quality.

- 30 = low quality. This corresponds to the p10 worst output of mozjpeg -quality 30.
- 50 = medium quality. This corresponds to the average output of cjxl -q 40 or mozjpeg -quality 40, or the p10 output of cjxl -q 50 or mozjpeg -quality 60.
- 70 = high quality. This corresponds to the average output of cjxl -q 65 or mozjpeg -quality 70, p10 output of cjxl -q 75 or mozjpeg -quality 80.
- 90 = very high quality. Likely impossible to distinguish from the original when viewed at 1:1 from a normal viewing distance. This corresponds to the average output of mozjpeg -quality 95 or the p10 output of cjxl -q 

## Required packages for video support:

### Arch

```bash
sudo pacman -S vapoursynth vapoursynth-plugin-lsmashsource gcc make cmake pkg-config ttf-bitstream-vera # Keep install dependencies
```

### Other Linux

See http://www.vapoursynth.com/doc/installation.html#linux-installation

Install l-smash from https://github.com/l-smash/l-smash
Install LSMASHSource VapourSynth plugin from https://github.com/AkarinVS/L-SMASH-Works

### Windows

See http://www.vapoursynth.com/doc/installation.html#windows-installation

Then download the latest release-x86_64-cachedir-cwd.zip from https://github.com/AkarinVS/L-SMASH-Works/releases/tag/vA.3j

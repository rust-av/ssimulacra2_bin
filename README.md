# ssimulacra2_rs

[![Crates.io](https://img.shields.io/crates/v/ssimulacra2_rs?style=for-the-badge)](https://crates.io/crates/ssimulacra2_rs)
[![LICENSE](https://img.shields.io/crates/l/ssimulacra2_rs?style=for-the-badge)](https://github.com/rust-av/ssimulacra2_bin/blob/main/LICENSE)

Binary interface to the Rust implementation of the SSIMULACRA2 metric: <https://github.com/rust-av/ssimulacra2>

## Docker

First build the image

```bash
docker build -t ssimulacra2_bin .
```

Then run it:

```bash
docker run --rm -v $(PWD):/files ssimulacra2_bin image /files/source.png /files/distorted.png
```

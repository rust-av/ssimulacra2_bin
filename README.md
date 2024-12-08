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

Do not install or download any pre-release.
1) Follow Vapoursynth's installation step http://www.vapoursynth.com/doc/installation.html#windows-installation (Not the portable installation)
2) Get latest version of VapourSynth-x64-R##.exe
3) Run the .exe file to install VapourSynth, don't modify or change any setting if you are not familar with it
4) After Vapoursynth is installed, find C:\Users\<username>\AppData\Local\Programs\VapourSynth\ (REPLACE <username> WITH YOUR)
5) Then download the latest release-x86_64-cachedir-cwd.zip from https://github.com/AkarinVS/L-SMASH-Works/releases/tag/vA.3j
6) Decompress the release-x86_64-cachedir-cwd.zip, copy and paste the libvslsmashsource.dll to C:\Users\<username>\AppData\Local\Programs\VapourSynth\plugins
7) Install Rust on https://www.rust-lang.org/tools/install,  
8) Open Powershell and run rustc --version to check if it has been installed
9) Copy your path C:\Users\<username>\AppData\Local\Programs\VapourSynth\sdk\lib64
10) Enter the command on Powershell with the copied path: $env:LIB="C:\Users<username>\vapoursynth-portable\sdk\lib64;$env:LIB"
11) If it fail because it require Visual Studio or Visual Studio Tools, you can download either of them.
    1. Download on https://visualstudio.microsoft.com/downloads/ Find the "Tools for Visual Studio" bar and download the "Remote Tools for Visual Studio 2022". Run it.
    2. Make sure Desktop Development with C++ is checked, leave the optional check installation alone and download it.
    3. Retry step 8 again after you reboot your PC.
13) Run ssimulacra2_rs -h to check if it's running.
14) You're done!

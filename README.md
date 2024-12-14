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
       1. If you intends to install Vapoursynth system-wide instead of local, Python will also need to be system-wide. 
3) Get latest version of VapourSynth-x64-R##.exe
4) Run the .exe file to install VapourSynth, don't modify or change any setting if you are not familar with it
5) After Vapoursynth is installed, find its path
       1.  Local - C:\Users\<username>\AppData\Local\Programs\VapourSynth\ (REPLACE <username> WITH YOUR)
       2.  System-wide - C:\VapourSynth\
7) Then download the latest release-x86_64-cachedir-cwd.zip from https://github.com/AkarinVS/L-SMASH-Works/releases/tag/vA.3j
8) Decompress the release-x86_64-cachedir-cwd.zip, copy and paste the libvslsmashsource.dll to C:\Path\to\VapourSynth\plugins\
9) Install Rust on https://www.rust-lang.org/tools/install,  
10) Open Powershell and run rustc --version to check if it has been installed
11) Copy the full path C:\Path\to\VapourSynth\sdk\lib64
12) Enter the command on Powershell with the copied path: $env:LIB="C:\Path\to\VapourSynth\sdk\lib64;$env:LIB"
13) If it fail because it require Visual Studio or Visual Studio Tools, you can download either of them.
    1. Download on https://visualstudio.microsoft.com/downloads/ Find the "Tools for Visual Studio" bar and download the "Remote Tools for Visual Studio 2022". Run it.
    2. Make sure Desktop Development with C++ is checked, leave the optional check installation alone and download it.
    3. Retry step 8 again after you reboot your PC.
14) Run ssimulacra2_rs -h to check if it's running.
15) You're done!

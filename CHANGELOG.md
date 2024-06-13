## Version 0.5.2

- Re-remove harmonic mean. Unfortunately it cannot be used on sets that have negative numbers, which includes
  ssimulacra2 scores. Geometric mean was also tested, and in _theory_ it may be computed on sets with negative values,
  in practice it cannot be due to computer algorithms using logarithms for the computation.

## Version 0.5.1

- Add harmonic mean output
- Add ttf feature to `plotters` library
- Bump all dependencies

## Version 0.5.0

- Bump minimum MSRV to 1.74.1
- Add rolling average to progress bar
- Add increment parameter for skipping video frames
- Bump all dependencies

## Version 0.4.1

- Add support for piping y4m input from stdin as one of the sources

## Version 0.4.0

- Update to [version 2.1 of the metric](https://github.com/cloudinary/ssimulacra2/compare/v2.0...v2.1)

## Version 0.3.5

- Minor performance improvements

## Version 0.3.4

- Add `--frame-threads` argument to support frame parallel multithreading
    - This is set to 1 by default because it does linearly increase memory usage
- Various performance improvements

## Version 0.3.3

- Fix graphing

## Version 0.3.2

- Use crossterm instead of termion for Windows compatibility

## Version 0.3.1

- Add a progress bar
- Bump dependencies for a pretty healthy speed increase of about 20%

## Version 0.3.0

- Implement upstream changes from https://github.com/libjxl/libjxl/pull/1848

## Version 0.2.2

- Fix decoding of AV1 files with overlays when using `ffmpeg_build` feature
    - Or at least attempt to. There still seems to be some flakiness with linking libdav1d.
- Add support for YUVJ files

## Version 0.2.1

- Fix compilation with no default features

## Version 0.2.0

- Add support for video comparison
- This change splits ssimulacra2_rs into two subcommands:
    - `ssimulacra2_rs image input.png output.png` for comparing still images. All popular input formats are supported.
    - `ssimulacra2_rs video input.y4m output.y4m` for comparing videos. All popular input formats are supported. This
      feature requires the `ffmpeg` feature be enabled, which is on by default.

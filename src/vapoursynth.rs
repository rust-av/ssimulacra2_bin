use anyhow::{ensure, Result};
use ssimulacra2::Pixel;
use std::{
    mem::{size_of, transmute},
    path::Path,
};
use v_frame::prelude::ChromaSampling;
use vapoursynth::{format::Format, prelude::*, video_info::Resolution};

pub struct VapoursynthDecoder {
    env: Environment,
    cur_frame: usize,
}

impl VapoursynthDecoder {
    pub fn new(filename: &Path) -> Result<Self> {
        let script = format!(
            r#"
import vapoursynth as vs

core = vs.core

clip = core.lsmas.LWLibavSource(source="{}")
clip.set_output(0)
        "#,
            filename.canonicalize().unwrap().to_string_lossy()
        );
        let env = Environment::from_script(&script)?;
        Ok(Self { env, cur_frame: 0 })
    }

    fn get_node(&self) -> Result<Node<'_>> {
        Ok(self.env.get_output(0)?.0)
    }

    pub fn get_resolution(&self) -> Result<Resolution> {
        match self.get_node()?.info().resolution {
            Property::Constant(res) => Ok(res),
            Property::Variable => Err(anyhow::anyhow!(
                "Variable resolution videos are not supported"
            )),
        }
    }

    pub fn get_format(&self) -> Result<Format<'_>> {
        match self.get_node()?.info().format {
            Property::Constant(format) => Ok(format),
            Property::Variable => Err(anyhow::anyhow!("Variable format videos are not supported")),
        }
    }

    pub fn get_frame_count(&self) -> Result<usize> {
        Ok(self.get_node()?.info().num_frames)
    }

    pub fn read_video_frame<T: Pixel>(&mut self) -> Result<ssimulacra2::Frame<T>> {
        let format = self.get_format()?;
        assert!(format.bytes_per_sample() == size_of::<T>() as u8);
        ensure!(
            format.sample_type() == SampleType::Integer,
            "Currently only integer input is supported"
        );

        let res = self.get_resolution()?;
        let chroma = match (
            format.color_family(),
            format.sub_sampling_w() + format.sub_sampling_h(),
        ) {
            (ColorFamily::Gray, _) => ChromaSampling::Cs400,
            (_, 0) => ChromaSampling::Cs444,
            (_, 1) => ChromaSampling::Cs422,
            _ => ChromaSampling::Cs420,
        };
        let mut f: ssimulacra2::Frame<T> =
            ssimulacra2::Frame::new_with_padding(res.width, res.height, chroma, 0);

        {
            let frame = self.get_node()?.get_frame(self.cur_frame)?;
            match size_of::<T>() {
                1 => {
                    for (out_row, in_row) in f.planes[0]
                        .rows_iter_mut()
                        .zip((0..res.height).map(|y| frame.plane_row::<u8>(0, y)))
                    {
                        // SAFETY: We know that `T` is `u8` here.
                        out_row.copy_from_slice(unsafe { transmute(in_row) });
                    }
                    if format.color_family() != ColorFamily::Gray {
                        for (out_row, in_row) in f.planes[1].rows_iter_mut().zip(
                            (0..(res.height >> format.sub_sampling_h()))
                                .map(|y| frame.plane_row::<u8>(1, y)),
                        ) {
                            // SAFETY: We know that `T` is `u8` here.
                            out_row.copy_from_slice(unsafe { transmute(in_row) });
                        }
                    }
                    if format.color_family() != ColorFamily::Gray {
                        for (out_row, in_row) in f.planes[2].rows_iter_mut().zip(
                            (0..(res.height >> format.sub_sampling_h()))
                                .map(|y| frame.plane_row::<u8>(2, y)),
                        ) {
                            // SAFETY: We know that `T` is `u8` here.
                            out_row.copy_from_slice(unsafe { transmute(in_row) });
                        }
                    }
                }
                2 => {
                    for (out_row, in_row) in f.planes[0]
                        .rows_iter_mut()
                        .zip((0..res.height).map(|y| frame.plane_row::<u16>(0, y)))
                    {
                        // SAFETY: We know that `T` is `u16` here.
                        out_row.copy_from_slice(unsafe { transmute(in_row) });
                    }
                    if format.color_family() != ColorFamily::Gray {
                        for (out_row, in_row) in f.planes[1].rows_iter_mut().zip(
                            (0..(res.height >> format.sub_sampling_h()))
                                .map(|y| frame.plane_row::<u16>(1, y)),
                        ) {
                            // SAFETY: We know that `T` is `u16` here.
                            out_row.copy_from_slice(unsafe { transmute(in_row) });
                        }
                    }
                    if format.color_family() != ColorFamily::Gray {
                        for (out_row, in_row) in f.planes[2].rows_iter_mut().zip(
                            (0..(res.height >> format.sub_sampling_h()))
                                .map(|y| frame.plane_row::<u16>(2, y)),
                        ) {
                            // SAFETY: We know that `T` is `u16` here.
                            out_row.copy_from_slice(unsafe { transmute(in_row) });
                        }
                    }
                }
                _ => unreachable!(),
            }
        }

        self.cur_frame += 1;
        Ok(f)
    }
}

//!
//! This module implements the Short Time Fourier Transformation as described here:
//! http://eeweb.poly.edu/iselesni/EL713/STFT/stft_inverse.pdf
//!

use std::*;
use dft;
use sample::{self, Frame, Sample};
use num_complex::Complex;
use ::audio;

fn window_scalars(window_size: usize) -> Vec<f64> {
    // TODO: Using a sine is not sufficient for an overlap which is not half the window size!
    (0..window_size).map(|n| {
        f64::sin(f64::consts::PI / window_size as f64 * (n as f64 + 0.5))
    }).collect()
}

pub trait Stft: iter::Iterator<Item=Vec<Vec<Complex<f64>>>> + Sized {
    type NumChannels: sample::frame::NumChannels;
    fn sample_rate(&self) -> u32;
    fn window_size(&self) -> usize;
    fn window_overlap(&self) -> usize;
    /// Reconstructs a discrete signal from the STFT using an inverse Fourier transformation.
    fn inverse<O>(self) -> Inverse<Self, O>
        where O: sample::Frame<NumChannels=Self::NumChannels>,
              O::Sample: sample::FromSample<f64> + sample::FromSample<<O::Float as sample::Frame>::Sample>,
              <<O::Float as sample::Frame>::Sample as sample::Sample>::Float: sample::ToSample<f64> {
        let window_size = self.window_size();
        let windows = vec![
            collections::VecDeque::new(),
            vec![O::Float::equilibrium(); window_size].into_iter().collect(),
        ];
        let window_scalars = window_scalars(window_size).into_iter()
            .map(|s| s.powi(2))
            .collect();
        Inverse {
            stft: self,
            fft_plan: dft::Plan::new(dft::Operation::Inverse, window_size),
            window_scalars: window_scalars,
            windows: windows.into_iter().collect(),
        }
    }
}


pub struct Inverse<T, O>
    where T: Stft + Sized,
          O: sample::Frame,
          O::Sample: sample::FromSample<f64> + sample::FromSample<<O::Float as sample::Frame>::Sample>,
          <<O::Float as sample::Frame>::Sample as sample::Sample>::Float: sample::ToSample<f64> {
    stft: T,
    fft_plan: dft::Plan<f64>,
    window_scalars: Vec<f64>,
    windows: collections::VecDeque<collections::VecDeque<O::Float>>,
}

impl<T, O> iter::Iterator for Inverse<T, O>
    where T: Stft<NumChannels=O::NumChannels> + Sized,
          O: sample::Frame,
          O::Sample: sample::FromSample<f64> + sample::FromSample<<O::Float as sample::Frame>::Sample>,
          <<O::Float as sample::Frame>::Sample as sample::Sample>::Float: sample::ToSample<f64> {
    type Item = O;
    fn next(&mut self) -> Option<Self::Item> {
        assert_eq!(2, self.windows.len());
        if self.windows.front().unwrap().len() == 0 {
            let mut blocks = match self.stft.next() {
                Some(blocks) => blocks,
                None => return None,
            };
            for block in &mut blocks {
                dft::transform(block, &self.fft_plan);
            }
            assert_eq!(O::n_channels(), blocks.len());
            debug_assert!(blocks.iter().all(|block| block.len() == self.stft.window_size()));

            let next_window = (0..self.stft.window_size())
                .map(|n| O::Float::from_fn(|ch| {
                    <O::Sample as sample::Sample>::Float::from_sample(blocks[ch][n].re)
                }))
                .collect();
            self.windows.push_back(next_window);
            self.windows.pop_front();
            // Drop the overlapping part of the previous frame.
            self.windows.front_mut().unwrap()
                .drain(0..self.stft.window_size() - self.stft.window_overlap());
        }

        let n = self.stft.window_overlap() - self.windows.front().unwrap().len();
        let sa = self.window_scalars[n + self.stft.window_overlap()];
        let sb = self.window_scalars[n];
        // The sum of sa and sb should be equal to 1.0.
        debug_assert!(1.0-10e-9 <= sa+sb && sa+sb <= 1.0+10e-9);

        let fa = self.windows.front_mut().unwrap().pop_front().unwrap();
        let fb = self.windows.back().unwrap()[n];

        Some(fa.zip_map(fb, |a, b| O::Sample::from_sample(a * sa.to_sample() + b * sb.to_sample())))
    }
}

impl<T, O> audio::Source for Inverse<T, O>
    where T: Stft<NumChannels=O::NumChannels> + Sized,
          O: sample::Frame,
          O::Sample: sample::FromSample<f64> + sample::FromSample<<O::Float as sample::Frame>::Sample>,
          <<O::Float as sample::Frame>::Sample as sample::Sample>::Float: sample::ToSample<f64> {
    fn sample_rate(&self) -> u32 {
        self.stft.sample_rate()
    }
}


pub struct FromSource<S>
    where S: audio::Source,
          S::Item: sample::Frame,
          <S::Item as sample::Frame>::Sample: sample::ToSample<f64> {
    input: S,
    window_size: usize,
    overlap: usize,
    fft_plan: dft::Plan<f64>,
    /// Stores the previous window.
    window: collections::VecDeque<S::Item>,
    window_scalars: Vec<f64>,
}

impl<S> Stft for FromSource<S>
    where S: audio::Source,
          S::Item: sample::Frame,
          <S::Item as sample::Frame>::Sample: sample::ToSample<f64> {
    type NumChannels = <S::Item as sample::Frame>::NumChannels;
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    fn window_size(&self) -> usize {
        self.window_size
    }

    fn window_overlap(&self) -> usize {
        self.overlap
    }
}

impl<S> iter::Iterator for FromSource<S>
    where S: audio::Source,
          S::Item: sample::Frame,
          <S::Item as sample::Frame>::Sample: sample::ToSample<f64> {
    type Item = Vec<Vec<Complex<f64>>>;
    fn next(&mut self) -> Option<Self::Item> {
        assert_eq!(self.window_size, self.window.len());
        for i in 0..(self.window_size - self.overlap) {
            let frame = match self.input.next() {
                Some(frame) => frame,
                None if i == 0 => return None,
                None => S::Item::equilibrium(),
            };
            self.window.pop_front();
            self.window.push_back(frame);
        }
        assert_eq!(self.window_size, self.window.len());

        let blocks: Vec<_> = (0..S::Item::n_channels())
            .map(|ch| {
                let mut block: Vec<_> = self.window.iter().zip(self.window_scalars.iter())
                    .map(|(frame, scalar)| {
                        let sample: f64 = frame.channel(ch).unwrap().to_sample();
                        Complex::new(sample * scalar, 0.0)
                    }).collect();
                dft::transform(&mut block, &self.fft_plan);
                block
            })
            .collect();
        assert_eq!(S::Item::n_channels(), blocks.len());
        Some(blocks)
    }
}

pub trait IntoStft: audio::Source + Sized
    where Self::Item: sample::Frame,
          <Self::Item as sample::Frame>::Sample: sample::ToSample<f64> {
    /// Computes the Short Time Fourier Transform of the signal over periods specified by the
    /// number of samples. The window size should be a power of two.
    ///
    /// The windows will overlap by 50%. The first and last windows contain a zero padding.
    fn stft(self, window_size: usize) -> FromSource<Self> {
        let window = iter::repeat(Self::Item::equilibrium())
            .take(window_size)
            .collect();
        FromSource {
            input: self,
            window_size: window_size,
            overlap: window_size / 2, // TODO: See the window_scalars function.
            fft_plan: dft::Plan::new(dft::Operation::Forward, window_size),
            window: window,
            window_scalars: window_scalars(window_size),
        }
    }
}

impl<T> IntoStft for T
    where T: audio::Source,
          T::Item: sample::Frame,
          <T::Item as sample::Frame>::Sample: sample::ToSample<f64> { }

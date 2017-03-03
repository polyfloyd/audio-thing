use std::*;
use ::filter::stft;

pub struct PhaseVocoder<S>
    where S: stft::Stft {
    input: S,
    ratio: f64,
    // Stores how much of the accum[0] has been consumed in previous iterations.
    consumption: f64,
    accum: collections::VecDeque<Vec<Vec<f64>>>,
}

impl<S> iter::Iterator for PhaseVocoder<S>
    where S: stft::Stft {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        assert!(0.0 <= self.consumption && self.consumption < 1.0);

        // The ratio is also the number of blocks that we should use.
        let num_blocks = (self.consumption + self.ratio).ceil() as usize;
        assert!(num_blocks >= 1);

        self.accum.extend(self.input.by_ref().take(self.ratio.ceil() as usize));
        let num_blocks_available = cmp::min(num_blocks, self.accum.len());
        if num_blocks_available == 0 {
            return None;
        }

        let num_channels = self.accum[0].len();
        let output_block = self.accum.iter()
            .take(num_blocks_available)
            .fold(vec![vec![0.0; self.input.window_size()]; num_channels], |mut sum, block| {
                let avg = 1.0 / num_blocks_available as f64;
                for (ch, channel) in block.into_iter().enumerate() {
                    for (i, bin_half) in channel.into_iter().enumerate() {
                        // TODO: Better interpolation between blocks.
                        sum[ch][i] += bin_half * avg;
                    }
                }
                sum
            });

        // Remove the blocks that have been fully consumed.
        self.consumption = self.consumption + self.ratio;
        self.accum.drain(0..self.consumption.floor() as usize);
        self.consumption %= 1.0;

        Some(output_block)
    }
}

impl<S> stft::Stft for PhaseVocoder<S>
    where S: stft::Stft {
    type NumChannels = S::NumChannels;
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    fn window_size(&self) -> usize {
        self.input.window_size()
    }

    fn window_overlap(&self) -> usize {
        self.input.window_overlap()
    }
}

pub trait AdjustTempo: stft::Stft + Sized {
    fn adjust_tempo(self, ratio: f64) -> PhaseVocoder<Self> {
        assert!(ratio > 0.0);
        PhaseVocoder {
            input: self,
            ratio: ratio,
            consumption: 0.0,
            accum: collections::VecDeque::new(),
        }
    }
}

impl<T> AdjustTempo for T
    where T: stft::Stft { }

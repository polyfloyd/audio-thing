use std::*;
use ::filter::stft;

pub struct PhaseVocoder<S>
    where S: stft::Stft {
    input: S,
    ratio: f64,
    counter: f64,
    accum: collections::VecDeque<Vec<Vec<f64>>>,
    output: collections::VecDeque<Vec<Vec<f64>>>,
}

impl<S> iter::Iterator for PhaseVocoder<S>
    where S: stft::Stft {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if self.output.len() == 0 {
            while self.counter < 1.0 {
                match self.input.next() {
                    Some(block) => {
                        self.accum.push_back(block);
                        self.counter += 1.0 / self.ratio;
                    },
                    None => return None, // TODO: process leftovers.
                };
            }
            assert!(self.accum.len() > 0);

            if self.ratio < 1.0 {
                // Slow down the tempo. Input blocks are replicated to create output blocks.
                let num_blocks = (1.0 / self.ratio).floor() as usize;
                for block in iter::repeat(self.accum.pop_front().unwrap()).take(num_blocks) {
                    self.output.push_back(block);
                }

            } else if self.ratio >= 1.0 {
                // Accelerate the tempo. Input blocks are merged into less blocks.
                let num_channels = self.accum[0].len();
                let window_size = self.input.window_size();
                let avg = 1.0 / self.accum.len() as f64;
                let block = self.accum
                    .drain(..)
                    .fold(vec![vec![0.0; window_size]; num_channels], |mut sum, block| {
                        for (ch, channel) in block.into_iter().enumerate() {
                            for (i, bin_half) in channel.into_iter().enumerate() {
                                sum[ch][i] += bin_half * avg;
                            }
                        }
                        sum
                    });
                self.output.push_back(block);
            }
            self.counter %= 1.0;
        }

        Some(self.output.pop_front().unwrap())
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
            counter: 0.0,
            accum: collections::VecDeque::new(),
            output: collections::VecDeque::new(),
        }
    }
}

impl<T> AdjustTempo for T
    where T: stft::Stft { }

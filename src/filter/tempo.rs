use std::*;
use ::filter::stft;

pub struct PhaseVocoder<S>
    where S: stft::Stft {
    pub ratio: sync::Arc<sync::Mutex<f64>>,

    input: S,
    // Stores how much of the accum[0] has been consumed in previous iterations.
    consumption: f64,
    accum: collections::VecDeque<Vec<Vec<f64>>>,
}

impl<S> iter::Iterator for PhaseVocoder<S>
    where S: stft::Stft {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        assert!(0.0 <= self.consumption && self.consumption < 1.0);

        let ratio = self.ratio.lock().unwrap();
        assert!(*ratio > 0.0);

        // The ratio is also the number of blocks that we should use.
        let next_consumption = self.consumption + *ratio;

        self.accum.extend(self.input.by_ref().take(next_consumption.ceil() as usize));

        let block_index = (next_consumption / 2.0).floor() as usize;
        if block_index >= self.accum.len() {
            assert_eq!(None, self.input.next());
            return None;
        }
        let output_block = self.accum[block_index].clone();

        // Remove the blocks that have been fully consumed.
        let num_blocks_available = self.accum.len();
        self.accum.drain(0..cmp::min(next_consumption.floor() as usize, num_blocks_available));
        self.consumption = next_consumption % 1.0;

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
    fn adjust_tempo(self, ratio: sync::Arc<sync::Mutex<f64>>) -> PhaseVocoder<Self> {
        assert!(*ratio.lock().unwrap() > 0.0);
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

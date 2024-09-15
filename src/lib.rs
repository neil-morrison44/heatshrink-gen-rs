#![feature(gen_blocks)]

use core::panic;
use core::pin::{pin, Pin};
mod offset_reader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HSDState {
    TagBit,
    YieldLiteral,
    BackRefIndexMsb,
    BackRefIndexLsb,
    BackRefCountMsb,
    BackRefCountLsb,
    YieldBackRef,
    Unknown,
}

impl HSDState {
    fn from_input(value: Option<u8>) -> Option<Self> {
        match value {
            Some(v) => Some(match v {
                0 => HSDState::TagBit,
                1 => HSDState::YieldLiteral,
                2 => HSDState::BackRefIndexMsb,
                3 => HSDState::BackRefIndexLsb,
                4 => HSDState::BackRefCountMsb,
                5 => HSDState::BackRefCountLsb,
                6 => HSDState::YieldBackRef,
                _ => HSDState::Unknown,
            }),
            None => None,
        }
    }
}

pub struct HeatShrink<const W: usize, const L: usize> {
    window: [u8; W],
    lookahead: [u8; L],
    window_index: usize,
    lookahead_index: usize,
    bit_offset: usize,
}

impl<const W: usize, const L: usize> HeatShrink<W, L> {
    // Constructor: initialize the window and lookahead buffers to zero
    pub fn new() -> Self {
        Self {
            window: [0; W],
            lookahead: [0; L],
            bit_offset: 0,
            window_index: 0,
            lookahead_index: 0,
        }
    }

    pub fn decode<I: Iterator<Item = u8>>(&mut self, mut input: I) -> impl Iterator<Item = u8> {
        let or = offset_reader::OffsetReader::new();

        gen move {
            // Need to pin something to fix this, or something.
            let mut offset_input = or.with_offset(input, None);

            while let Some(byte) = offset_input.next() {
                let bit = 1 << 0;
                or.advance_offset();

                match bit {
                    1 => {
                        // Literal Byte
                        let byte = offset_input.next().unwrap();
                        yield self.prep_output_byte(byte);
                    }
                    0 => {
                        // BackRef
                        // TODO: only one of these will be used when the window size is small (and can be addressed by 1 byte)
                        let msb_index_byte: usize = offset_input.next().unwrap().into();
                        let lsb_index_byte: usize = offset_input.next().unwrap().into();

                        let back_ref_index: usize = (msb_index_byte << 8) | lsb_index_byte;

                        let msb_count_byte: usize = offset_input.next().unwrap().into();
                        let lsb_count_byte: usize = offset_input.next().unwrap().into();

                        let count: usize = (msb_count_byte << 8) | lsb_count_byte;

                        for _ in 0..count {
                            // since we always add 1 to the window index when we output the back_ref_index doesn't need to change
                            let window_value = self.get_window_value(back_ref_index);
                            yield self.prep_output_byte(window_value);
                        }
                    }
                    _ => {
                        panic!("A bit was set to something other than 0 or 1");
                    }
                }
            }
        }
    }

    fn prep_output_byte(&mut self, byte: u8) -> u8 {
        self.window[self.window_index] = byte;
        self.window_index += 1;
        if self.window_index == W {
            self.window_index = 0;
        }

        byte
    }

    fn get_window_value(&self, back_index: usize) -> u8 {
        if self.window_index > back_index {
            // no need to loop around
            return self.window[self.window_index - back_index];
        } else {
            let remainer = self.window_index - back_index;
            return self.window[W - remainer];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough() {
        let input: Vec<u8> = vec![0, 1, 2, 3];
        let mut hs = HeatShrink::<13, 4>::new();

        let out = hs.decode(input.into_iter());

        let result: Vec<u8> = out.collect();
        assert_eq!(result, vec![0, 1, 2, 3]);
    }
}
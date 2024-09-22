#![feature(gen_blocks)]

use bits_bytes_iter::BitsBytesIter;
use core::panic;

mod bits_bytes_iter;

pub struct HeatShrink<const W: usize, const L: usize> {
    window: [u8; W],
    lookahead: [u8; L],
    window_index: usize,
    lookahead_index: usize,
}

impl<'a, const W: usize, const L: usize> HeatShrink<W, L> {
    // Constructor: initialize the window and lookahead buffers to zero
    pub fn new() -> Self {
        Self {
            window: [0; W],
            lookahead: [0; L],
            window_index: 0,
            lookahead_index: 0,
        }
    }

    pub fn decode<I: Iterator<Item = &'a u8>>(&mut self, mut input: I) -> impl Iterator<Item = u8> {
        gen move {
            let mut bb_iter = BitsBytesIter::new(input);

            let mut loop_count: usize = 0;
            while let Some(bit) = bb_iter.next_bit() {
                match bit {
                    true => {
                        dbg!(&bb_iter);

                        if let Some(byte) = bb_iter.next() {
                            // dbg!(byte);
                            if byte == 186 {
                                println!("---- WRONG VALUE ----");
                                dbg!(&bb_iter);
                            }
                            yield self.prep_output_byte(byte);
                        } else {
                            return;
                        }
                    }
                    false => {
                        let back_ref_index = if W > 255 {
                            let msb_index_byte = bb_iter.next();
                            let lsb_index_byte = bb_iter.next();

                            if let [Some(msb_index_byte), Some(lsb_index_byte)] =
                                [msb_index_byte, lsb_index_byte]
                            {
                                let msb_index_byte: usize = msb_index_byte.into();
                                let lsb_index_byte: usize = lsb_index_byte.into();
                                let back_ref_index: usize = (msb_index_byte << 8) | lsb_index_byte;
                                back_ref_index
                            } else {
                                return;
                            }
                        } else {
                            let back_ref_index = bb_iter.next();

                            if let Some(back_ref_index) = back_ref_index {
                                back_ref_index.into()
                            } else {
                                return;
                            }
                        };

                        let mut count_bits: [bool; L] = [false; L];

                        for i in 0..L {
                            let bit = bb_iter.next_bit();
                            if let Some(bit) = bit {
                                count_bits[i] = bit;
                            } else {
                                return;
                            }
                        }

                        let count = self.read_number_from_bits(&mut count_bits) + 1;

                        for _ in 0..count {
                            // since we always add 1 to the window index when we output the back_ref_index doesn't need to change
                            let window_value = self.get_window_value(back_ref_index);
                            yield self.prep_output_byte(window_value);
                        }

                        if loop_count > 1 {
                            return;
                        }
                        loop_count += 1;
                        // return;
                    }
                }
            }
        }
    }

    fn read_number_from_bits(&mut self, bits: &mut [bool; L]) -> usize {
        let mut result = 0;

        for (i, &bit) in bits.iter().enumerate() {
            if bit {
                result |= 1 << (L - 1 - i);
            }
        }

        result
    }

    fn prep_output_byte(&mut self, byte: u8) -> u8 {
        self.window[self.window_index] = byte;
        self.window_index += 1;
        if self.window_index == (W - 1) {
            self.window_index = 0;
        }

        byte
    }

    fn get_window_value(&self, back_index: usize) -> u8 {
        if self.window_index >= back_index {
            return self.window[self.window_index - back_index];
        } else {
            let remainer = self.window_index % back_index;
            return self.window[(W - 1) - remainer];
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use super::*;

    #[test]
    fn passthrough_bytes() {
        let first: u8 = 0b11000000;
        let second: u8 = 0b11100000;
        let input: Vec<&u8> = vec![&first, &second];
        let mut hs = HeatShrink::<13, 4>::new();

        let out = hs.decode(input.into_iter());

        let result: Vec<u8> = out.collect();
        assert_eq!(result, vec![129, 128]);
    }

    #[test]
    fn compare_loop() {
        let input = include_bytes!("../tsz-compressed-data.bin.hs");
        let mut hs = HeatShrink::<256, 8>::new();

        let input_iter = (*input).iter();

        let mut out = hs.decode(input_iter);

        let expected_output = include_bytes!("../tsz-compressed-data.bin");
        let mut expected_iter = expected_output.iter();

        while let Some(expected) = expected_iter.next() {
            let actual = out.next().unwrap();

            assert_eq!(actual, *expected);
        }
    }

    #[test]
    fn decode_bytes() {
        let input = include_bytes!("../tsz-compressed-data.bin.hs");
        let mut hs = HeatShrink::<256, 8>::new();

        let input_iter = (*input).iter();

        let out = hs.decode(input_iter);

        let result: Vec<u8> = out.collect();

        let mut file = File::create("./test-output.bin").unwrap();
        file.write_all(&result).unwrap();

        // let expected_output = include_bytes!("../tsz-compressed-data.bin");
        // assert_eq!(result, expected_output);
    }
}

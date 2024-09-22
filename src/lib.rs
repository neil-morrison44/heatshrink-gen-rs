#![feature(gen_blocks)]

use bits_bytes_iter::BitsBytesIter;
use byte_buffer::ByteBuffer;
use core::panic;
use heapless::Deque;

mod bits_bytes_iter;
mod byte_buffer;

pub struct HeatShrink<const W: usize, const L: usize> {
    window: [u8; W],
    lookahead: [u8; L],
    window_index: usize,
    lookahead_index: usize,
}

impl<'a, const W: usize, const L: usize> HeatShrink<W, L> {
    pub fn new() -> Self {
        Self {
            window: [0; W],
            lookahead: [0; L],
            window_index: 0,
            lookahead_index: 0,
        }
    }

    pub fn reset(&mut self) -> () {
        self.window = [0; W];
        self.lookahead = [0; L];
        self.window_index = 0;
        self.lookahead_index = 0;
    }

    pub fn encode<I: Iterator<Item = &'a u8>>(&mut self, mut input: I) -> impl Iterator<Item = u8> {
        gen move {
            let mut lookahead_deque = Deque::<_, L>::new();
            let mut byte_buffer = ByteBuffer::new();

            // while let Some(input_byte) = input.next() {

            if let Some(input_byte) = input.next() {
                lookahead_deque.push_front(*input_byte).unwrap();
            }

            while !lookahead_deque.is_empty() {
                // fill the lookahead using the input
                while !lookahead_deque.is_full() {
                    if let Some(input_byte) = input.next() {
                        lookahead_deque.push_front(*input_byte).unwrap();
                    } else {
                        break;
                    }
                }

                // Look through the window from the current window index backwards
                // Find the largest bytes which match

                if false {
                    // - prepare to output a 0 bit

                    if let Some(output_byte) = byte_buffer.add_bit(false) {
                        yield output_byte;
                    }
                    // - the backref index byte / bytes
                    // - the count bits
                } else {
                    // - prepare to output a 1 bit
                    // - and the byte literal
                    if let Some(output_byte) = byte_buffer.add_bit(true) {
                        yield output_byte;
                    }

                    let literal_byte = lookahead_deque.pop_back().unwrap();
                    if let Some(output_byte) = byte_buffer.add_byte(literal_byte) {
                        yield output_byte;
                    }
                    self.push_window_value(literal_byte);
                }
            }

            // deal with what's left in the lookahead buffer

            while let Some(byte) = lookahead_deque.pop_back() {
                if let Some(output_byte) = byte_buffer.add_bit(true) {
                    yield output_byte;
                }
                if let Some(output_byte) = byte_buffer.add_byte(byte) {
                    yield output_byte;
                }
            }

            return;

            // check the window for a > 2 (or 3, depends on W & L values) byte match from the first lookahead onwards
            // if there's a match for a count of N bytes then:
            // - prepare to output a 0 bit
            // - the backref index byte / bytes
            // - the count bits

            // put the processed byte into the window & shift the window index along one, repeat
        }
    }

    pub fn decode<I: Iterator<Item = &'a u8>>(&mut self, input: I) -> impl Iterator<Item = u8> {
        gen move {
            let mut bb_iter = BitsBytesIter::new(input);

            while let Some(bit) = bb_iter.next_bit() {
                match bit {
                    true => {
                        if let Some(byte) = bb_iter.next() {
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
                            // since we always add 1 to the window index when we output
                            // the back_ref_index doesn't need to change
                            let window_value = self.get_window_value(back_ref_index);
                            yield self.prep_output_byte(window_value);
                        }
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

    fn push_window_value(&mut self, byte: u8) -> () {
        self.window[self.window_index] = byte;
        self.window_index += 1;
        if self.window_index >= W {
            self.window_index = 0;
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
        let input = include_bytes!("../test_inputs/tsz-compressed-data.bin.hs");
        let mut hs = HeatShrink::<256, 8>::new();

        let input_iter = (*input).iter();
        let mut out = hs.decode(input_iter);

        let expected_output = include_bytes!("../test_inputs/tsz-compressed-data.bin");
        let mut expected_iter = expected_output.iter();

        let mut index = 0;

        while let Some(expected) = expected_iter.next() {
            let actual = out.next().unwrap();

            assert_eq!(actual, *expected, "Failed on index {index}");
            index += 1;
        }
    }

    #[test]
    fn decode_bytes() {
        let input = include_bytes!("../test_inputs/tsz-compressed-data.bin.hs");
        let mut hs = HeatShrink::<256, 8>::new();

        let input_iter = (*input).iter();

        let out = hs.decode(input_iter);

        let result: Vec<u8> = out.collect();

        let mut file = File::create("./test_output/test-output.bin").unwrap();
        file.write_all(&result).unwrap();

        // let expected_output = include_bytes!("../tsz-compressed-data.bin");
        // assert_eq!(result, expected_output);
    }

    #[test]
    fn encode_decode() {
        let input = include_bytes!("./lib.rs");
        let mut hs = HeatShrink::<256, 8>::new();

        let input_iter = (*input).iter();
        let encode_iter = hs.encode(input_iter);

        let encode_output: Vec<u8> = encode_iter.collect();

        let mut file = File::create("./test_output/test-output-rs.bin").unwrap();
        file.write_all(&encode_output).unwrap();

        hs.reset();

        let encode_output_iter = encode_output.iter();

        let out = hs.decode(encode_output_iter);

        let decoded_output: Vec<u8> = out.collect();

        let mut file = File::create("./test_output/test-output-rs-decoded.rs").unwrap();
        file.write_all(&decoded_output).unwrap();
        // assert_eq!(result, expected_output);
    }
}

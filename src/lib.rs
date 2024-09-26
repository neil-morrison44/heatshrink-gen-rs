#![feature(gen_blocks)]
#![feature(generic_const_exprs)]

use bits_bytes_iter::BitsBytesIter;
use byte_buffer::ByteBuffer;
use heapless::{Deque, HistoryBuffer};

mod bits_bytes_iter;
mod byte_buffer;

#[macro_export]
macro_rules! heatshrink {
    ($w:expr, $l:expr) => {
        HeatShrink::<$w, $l, { 1 << $w }, { 1 << $l }>
    };
}

pub struct HeatShrink<
    const W: usize,
    const L: usize,
    const WINDOW_SIZE: usize,
    const LOOKAHEAD_SIZE: usize,
> {
    window: HistoryBuffer<u8, WINDOW_SIZE>,
    lookahead: Deque<u8, LOOKAHEAD_SIZE>,
}

impl<'a, const W: usize, const L: usize, const WINDOW_SIZE: usize, const LOOKAHEAD_SIZE: usize>
    HeatShrink<W, L, WINDOW_SIZE, LOOKAHEAD_SIZE>
{
    pub fn new() -> Self {
        let mut hs = Self {
            window: HistoryBuffer::new(),
            lookahead: Deque::new(),
        };
        hs.reset();
        hs
    }

    pub fn reset(&mut self) -> () {
        self.window.clear_with(0);
        self.lookahead.clear();
    }

    pub fn encode<I: Iterator<Item = &'a u8>>(&mut self, mut input: I) -> impl Iterator<Item = u8> {
        macro_rules! yield_some_byte {
            ($c:expr) => {
                if let Some(output_byte) = $c {
                    yield output_byte;
                }
            };
        }

        gen move {
            let mut byte_buffer = ByteBuffer::new();

            if let Some(input_byte) = input.next() {
                self.lookahead.push_back(*input_byte).unwrap();
            }

            loop {
                while !self.lookahead.is_full() {
                    if let Some(input_byte) = input.next() {
                        self.lookahead.push_back(*input_byte).unwrap();
                    } else {
                        break;
                    }
                }

                if self.lookahead.is_empty() {
                    break;
                }

                let literal_byte = self.lookahead.pop_front().unwrap();

                if let Some((back_ref_index, count)) = self.find_lookahead_in_window(literal_byte) {
                    yield_some_byte!(byte_buffer.add_bit(false));

                    if W > 255 {
                        let msb_index_byte: u8 = (back_ref_index >> 8) as u8;
                        let lsb_index_byte: u8 = (back_ref_index & 0xFF) as u8;

                        yield_some_byte!(byte_buffer.add_byte(msb_index_byte));
                        yield_some_byte!(byte_buffer.add_byte(lsb_index_byte));
                    } else {
                        let back_ref_byte = back_ref_index as u8;
                        yield_some_byte!(byte_buffer.add_byte(back_ref_byte));
                    }

                    let bits = self.write_number_to_bits(count - 1);

                    for bit in bits {
                        yield_some_byte!(byte_buffer.add_bit(bit));
                    }

                    self.push_window_value(literal_byte);
                    for _ in 0..(count - 1) {
                        let byte = self.lookahead.pop_front().unwrap();
                        self.push_window_value(byte);
                    }
                } else {
                    yield_some_byte!(byte_buffer.add_bit(true));
                    yield_some_byte!(byte_buffer.add_byte(literal_byte));

                    self.push_window_value(literal_byte);
                }
            }
            return;
        }
    }

    fn find_lookahead_in_window(&self, literal_byte: u8) -> Option<(usize, usize)> {
        let mut max_match = None;
        let filled_window_count = self.window.oldest_ordered().into_iter().count();
        let mut window_iter = self.window.oldest_ordered().into_iter().enumerate();

        while let Some((index, &byte)) = window_iter.next() {
            if byte == literal_byte {
                let lookahead_iter = self.lookahead.iter();

                let current_count = 1 + window_iter
                    .by_ref()
                    .map(|(_, b)| b)
                    .zip(lookahead_iter)
                    .take_while(|(a, b)| a == b)
                    .count();

                if current_count > max_match.map_or(2, |(_, len)| len) {
                    max_match = Some(((filled_window_count - 1) - index, current_count));
                }
            }
        }

        max_match
    }

    pub fn decode<I: Iterator<Item = &'a u8>>(&mut self, input: I) -> impl Iterator<Item = u8> {
        gen move {
            let mut bb_iter = BitsBytesIter::new(input);

            while let Some(bit) = bb_iter.next_bit() {
                match bit {
                    true => {
                        if let Some(byte) = bb_iter.next() {
                            self.push_window_value(byte);
                            yield byte;
                        } else {
                            return;
                        }
                    }
                    false => {
                        let back_ref_index = if W > 8 {
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

                        for i in 0..count {
                            // since we always add 1 to the window index when we output
                            // the back_ref_index doesn't need to change
                            let output_byte = self.get_window_value(back_ref_index);
                            // dbg!(output_byte);
                            self.push_window_value(output_byte);
                            yield output_byte;
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

    fn write_number_to_bits(&self, number: usize) -> [bool; L] {
        let mut bits = [false; L];
        for i in 0..L {
            bits[i] = (number & (1 << (L - 1 - i))) != 0;
        }
        bits
    }

    fn get_window_value(&self, back_index: usize) -> u8 {
        let (first_slice, second_slice) = self.window.as_slices();

        if back_index < second_slice.len() {
            let index = (second_slice.len() - 1) - back_index;
            second_slice[index]
        } else {
            let index = (first_slice.len() - 1) - (back_index - second_slice.len());
            first_slice[index]
        }
    }

    fn push_window_value(&mut self, byte: u8) -> () {
        self.window.write(byte);
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
        let mut hs = <heatshrink!(13, 4)>::new();

        let out = hs.decode(input.into_iter());

        let result: Vec<u8> = out.collect();
        assert_eq!(result, vec![129, 128]);
    }

    #[test]
    fn compare_loop() {
        let input = include_bytes!("../test_inputs/tsz-compressed-data.bin.hs");
        let mut hs = <heatshrink!(8, 4)>::new();

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
        let mut hs = <heatshrink!(8, 4)>::new();

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
        let mut hs = <heatshrink!(8, 4)>::new();

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

    #[test]
    fn encode_decode_but_big() {
        let input = include_bytes!("./lib.rs");
        let mut hs = <heatshrink!(16, 8)>::new();

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

#![no_std]
#![feature(gen_blocks)]
#![allow(incomplete_features)]
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
        macro_rules! yield_byte {
            ($byte:expr) => {
                if let Some(output_byte) = $byte {
                    yield output_byte;
                }
            };
        }

        gen move {
            let mut byte_buffer = ByteBuffer::new();

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
                    yield_byte!(byte_buffer.add_bit(false));

                    if W > 8 {
                        let msb_index_byte: u8 = (back_ref_index >> 8) as u8;
                        let lsb_index_byte: u8 = (back_ref_index & 0xFF) as u8;
                        yield_byte!(byte_buffer.add_byte(msb_index_byte));
                        yield_byte!(byte_buffer.add_byte(lsb_index_byte));
                    } else {
                        yield_byte!(byte_buffer.add_byte(back_ref_index as u8));
                    }

                    let bits = self.write_number_to_bits(count - 1);
                    for bit in bits {
                        yield_byte!(byte_buffer.add_bit(bit));
                    }

                    self.push_window_value(literal_byte);
                    for _ in 0..(count - 1) {
                        let byte = self.lookahead.pop_front().unwrap();
                        self.push_window_value(byte);
                    }
                } else {
                    yield_byte!(byte_buffer.add_bit(true));
                    yield_byte!(byte_buffer.add_byte(literal_byte));

                    self.push_window_value(literal_byte);
                }
            }

            yield_byte!(byte_buffer.last_byte());

            return;
        }
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
                            // Read two bytes for the back reference index
                            match (bb_iter.next(), bb_iter.next()) {
                                (Some(msb), Some(lsb)) => ((msb as usize) << 8) | (lsb as usize),
                                _ => return, // If we can't read two bytes, exit
                            }
                        } else {
                            // Read one byte for the back reference index
                            if let Some(back_ref) = bb_iter.next() {
                                back_ref as usize
                            } else {
                                return;
                            }
                        };

                        let mut count_bits = [false; L];

                        for bit in count_bits.iter_mut() {
                            if let Some(next_bit) = bb_iter.next_bit() {
                                *bit = next_bit;
                            } else {
                                return;
                            }
                        }

                        let count = self.read_number_from_bits(&mut count_bits) + 1;

                        for _ in 0..count {
                            let output_byte = self.get_window_value(back_ref_index);
                            self.push_window_value(output_byte);
                            yield output_byte;
                        }
                    }
                }
            }
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

    fn read_number_from_bits(&self, bits: &[bool; L]) -> usize {
        bits.iter()
            .enumerate()
            .fold(0, |acc, (i, &bit)| acc | ((bit as usize) << (L - 1 - i)))
    }

    fn write_number_to_bits(&self, number: usize) -> [bool; L] {
        let mut bits = [false; L];
        (0..L).for_each(|i| bits[i] = (number >> (L - 1 - i)) & 1 != 0);
        bits
    }

    fn get_window_value(&self, back_index: usize) -> u8 {
        let (first, second) = self.window.as_slices();

        if let Some(index) = second
            .len()
            .checked_sub(1)
            .and_then(|len| len.checked_sub(back_index))
        {
            second[index]
        } else if let Some(index) = first
            .len()
            .checked_sub(1)
            .and_then(|len| len.checked_sub(back_index - second.len()))
        {
            first[index]
        } else {
            panic!("Back index out of bounds");
        }
    }

    #[inline]
    fn push_window_value(&mut self, byte: u8) -> () {
        self.window.write(byte);
    }
}

#[cfg(test)]

mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    #[test]
    fn passthrough_bytes() {
        let first: u8 = 0b11000000;
        let second: u8 = 0b11100000;
        let input: Vec<&u8> = std::vec![&first, &second];
        let mut hs = <heatshrink!(8, 4)>::new();

        let out = hs.decode(input.into_iter());

        let result: Vec<u8> = out.collect();
        assert_eq!(result, std::vec![129, 128]);
    }

    #[test]
    fn encode_decode() {
        let input = include_bytes!("./lib.rs");
        let mut hs = <heatshrink!(8, 4)>::new();

        let input_iter = (*input).iter();
        let encode_iter = hs.encode(input_iter);

        let encode_output: Vec<u8> = encode_iter.collect();

        // let mut file = File::create("./test_output/test-output-rs.bin").unwrap();
        // file.write_all(&encode_output).unwrap();

        hs.reset();

        let encode_output_iter = encode_output.iter();

        let out = hs.decode(encode_output_iter);

        let decoded_output: Vec<u8> = out.collect();

        // let mut file = File::create("./test_output/test-output-rs-decoded.rs").unwrap();
        // file.write_all(&decoded_output).unwrap();
        assert_eq!(input.to_vec(), decoded_output);
    }

    #[test]
    fn encode_decode_but_big() {
        let input = include_bytes!("./lib.rs");
        let mut hs = <heatshrink!(16, 8)>::new();

        let input_iter = (*input).iter();
        let encode_iter = hs.encode(input_iter);

        let encode_output: Vec<u8> = encode_iter.collect();

        // let mut file = File::create("./test_output/test-output-rs.bin").unwrap();
        // file.write_all(&encode_output).unwrap();

        hs.reset();

        let encode_output_iter = encode_output.iter();

        let out = hs.decode(encode_output_iter);

        let decoded_output: Vec<u8> = out.collect();

        // let mut file = File::create("./test_output/test-output-rs-decoded.rs").unwrap();
        // file.write_all(&decoded_output).unwrap();
        assert_eq!(input.to_vec(), decoded_output);
    }
}

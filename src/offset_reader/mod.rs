#![feature(gen_blocks)]

use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::{AcqRel, Relaxed};

pub struct OffsetReader {
    bit_offset: AtomicUsize, // 0 - 8
}

impl OffsetReader {
    // Constructor: initialize the window and lookahead buffers to zero
    pub fn new() -> Self {
        Self {
            bit_offset: AtomicUsize::new(0),
        }
    }

    pub fn advance_offset(&self) {
        self.bit_offset.fetch_add(1, AcqRel);
    }

    pub fn with_offset<I: Iterator<Item = u8>>(
        &self,
        mut input: I,
        start_byte: Option<u8>,
    ) -> impl Iterator<Item = u8> {
        gen move {
            let mut current_byte = match start_byte {
                Some(b) => b,
                None => input.next().unwrap(),
            };
            let mut maybe_next_byte = input.next();

            while let Some(next_byte) = maybe_next_byte {
                let offset = self.bit_offset.load(Relaxed);

                if offset == 8 {
                    // We've gone through a full byte, just shift the bits without yielding & reset to 0

                    current_byte = next_byte;
                    maybe_next_byte = input.next();
                    self.bit_offset.store(0, AcqRel);
                    continue;
                }

                if offset == 0 {
                    yield current_byte;
                } else {
                    yield ((current_byte << offset) | (next_byte >> (8 - offset)));
                }

                current_byte = next_byte;
                maybe_next_byte = input.next();
            }

            // TODO: maybe one last yield, not sure
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::offset_reader;

    use super::*;

    #[test]
    fn single_bytes() {
        let first: u8 = 0b10101010;
        let second: u8 = 0b11001100;

        let or = OffsetReader::new();

        or.advance_offset();
        or.advance_offset();
        or.advance_offset();

        let input = vec![first, second];
        let mut result_iter = or.with_offset(input.into_iter(), None);

        let result = result_iter.next();

        assert_eq!(result, Some(0b01010110));
    }

    #[test]
    fn with_a_first_byte() {
        let first: u8 = 0b10101010;
        let second: u8 = 0b11001100;

        let or = OffsetReader::new();
        or.advance_offset();
        or.advance_offset();
        or.advance_offset();

        let input = vec![second];
        let mut result_iter = or.with_offset(input.into_iter(), Some(first));

        let result = result_iter.next();

        assert_eq!(result, Some(0b01010110));
    }

    #[test]
    fn odd_then_even() {
        let zeros: u8 = 0b00000000;
        let ones: u8 = 0b11111111;

        let or = OffsetReader::new();
        or.advance_offset();
        or.advance_offset();
        or.advance_offset();
        or.advance_offset();

        let input = vec![
            zeros, ones, zeros, ones, zeros, ones, zeros, ones, zeros, ones, zeros, ones,
        ];
        let result_iter = or.with_offset(input.into_iter(), None);
        let result: Vec<u8> = result_iter.collect();

        assert_eq!(
            result,
            vec![15, 240, 15, 240, 15, 240, 15, 240, 15, 240, 15]
        );
    }

    #[test]
    fn zero_offset() {
        let zeros: u8 = 0b00000000;
        let ones: u8 = 0b11111111;

        let or = OffsetReader::new();

        let input = vec![
            zeros, ones, zeros, ones, zeros, ones, zeros, ones, zeros, ones, zeros, ones,
        ];
        let result_iter = or.with_offset(input.into_iter(), None);
        let result: Vec<u8> = result_iter.collect();

        assert_eq!(result, vec![0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0]);
    }

    #[test]
    fn changing_the_offset() {
        let zeros: u8 = 0b00000000;
        let ones: u8 = 0b11111111;

        let or = OffsetReader::new();

        let input = vec![
            zeros, ones, zeros, ones, zeros, ones, zeros, ones, zeros, ones, zeros, ones,
        ];
        let mut result_iter = or.with_offset(input.into_iter(), None);

        let result_one = result_iter.next();
        let result_two = result_iter.next();
        or.advance_offset();
        or.advance_offset();
        or.advance_offset();
        or.advance_offset();
        let result_three = result_iter.next();
        let result_four = result_iter.next();

        assert_eq!(
            vec![result_one, result_two, result_three, result_four],
            vec![Some(0), Some(255), Some(15), Some(240)]
        );
    }
}

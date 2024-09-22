use std::fmt::Debug;

pub struct BitsBytesIter<'a, I: Iterator<Item = &'a u8>> {
    bit_offset: u8,
    raw_iter: I,
    window: [Option<&'a u8>; 2],
}

impl<'a, I: Iterator<Item = &'a u8>> Debug for BitsBytesIter<'a, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BitsBytesIter")
            .field("bit_offset", &self.bit_offset)
            .field("window", &self.window)
            .finish()
    }
}

impl<'a, I: Iterator<Item = &'a u8>> Iterator for BitsBytesIter<'a, I> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        let byte = self.byte_from_window();
        self.load_next_from_raw();
        byte
    }
}

impl<'a, I: Iterator<Item = &'a u8>> BitsBytesIter<'a, I> {
    pub fn new(mut raw_iter: I) -> Self {
        let window = [raw_iter.next(), raw_iter.next()];
        Self {
            bit_offset: 0,
            raw_iter,
            window,
        }
    }

    pub fn next_bit(&mut self) -> Option<bool> {
        match self.byte_from_window() {
            Some(byte) => {
                self.advance_bit_offset();
                Some((byte & 0b10000000) != 0)
            }
            None => None,
        }
    }

    fn advance_bit_offset(&mut self) -> () {
        self.bit_offset += 1;
        if self.bit_offset == 8 {
            self.bit_offset = 0;
            self.load_next_from_raw();
        }
    }

    fn load_next_from_raw(&mut self) -> () {
        self.window[0] = self.window[1];
        let next_byte = self.raw_iter.next();
        self.window[1] = next_byte;
    }

    fn byte_from_window(&mut self) -> Option<u8> {
        match self.window {
            [None, None] => None,
            [None, Some(_)] => todo!(),
            [Some(current_byte), None] => {
                if self.bit_offset == 0 {
                    Some(*current_byte)
                } else {
                    let first_part = current_byte << self.bit_offset;
                    Some(first_part)
                }
            }
            [Some(current_byte), Some(next_byte)] => {
                if self.bit_offset == 0 {
                    return Some(*current_byte);
                }

                let first_part = current_byte << self.bit_offset;
                let second_part = next_byte >> (8 - self.bit_offset);
                Some(first_part | second_part)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bits_bytes_iter;

    use super::*;

    #[test]
    fn all_bits() {
        let first: u8 = 0b10101010;
        let second: u8 = 0b11001100;

        let input = vec![&first, &second];

        let mut or = BitsBytesIter::new(input.into_iter());

        let bits = [
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
            or.next_bit(),
        ];
        assert_eq!(
            bits,
            [
                Some(true),
                Some(false),
                Some(true),
                Some(false),
                Some(true),
                Some(false),
                Some(true),
                Some(false),
                Some(true),
                Some(true),
                Some(false),
                Some(false),
                Some(true),
                Some(true),
                Some(false),
                Some(false),
                None
            ]
        );
    }

    #[test]
    fn no_offset_bytes() {
        let first: u8 = 0b10101010;
        let second: u8 = 0b11001100;

        let input = vec![&first, &second];
        let mut or = BitsBytesIter::new(input.into_iter());

        let bytes = [or.next(), or.next(), or.next()];
        assert_eq!(bytes, [Some(0b10101010), Some(0b11001100), None]);
    }

    #[test]
    fn one_bit_then_bytes() {
        let first: u8 = 0b10101010;
        let second: u8 = 0b11001100;

        let input = vec![&first, &second];
        let mut or = BitsBytesIter::new(input.into_iter());

        let bit = or.next_bit();
        assert_eq!(bit, Some(true));

        let bytes = [or.next(), or.next(), or.next()];
        assert_eq!(bytes, [Some(0b01010101), Some(0b10011000), None]);
    }

    #[test]
    fn single_bytes() {
        let first: u8 = 0b10101010;
        let second: u8 = 0b11001100;

        let input = vec![&first, &second];
        let mut or = BitsBytesIter::new(input.into_iter());

        let bits: [Option<bool>; 4] = [or.next_bit(), or.next_bit(), or.next_bit(), or.next_bit()];
        assert_eq!(bits, [Some(true), Some(false), Some(true), Some(false)]);

        let result = [or.next(), or.next(), or.next()];

        assert_eq!(result, [Some(0b10101100), Some(0b11000000), None]);
    }

    #[test]
    fn big_file() {
        let input = include_bytes!("../../test_inputs/tsz-compressed-data.bin");

        let mut or = BitsBytesIter::new(input.into_iter());

        let result: Vec<u8> = or.collect();

        let expected_output = include_bytes!("../../test_inputs/tsz-compressed-data.bin");
        assert_eq!(result, expected_output);
    }

    #[test]
    fn big_file_bit_shifted() {
        let input = include_bytes!("../../test_inputs/tsz-compressed-data.bin");
        let mut or = BitsBytesIter::new(input.into_iter());

        or.next();
        for _ in 0..3 {
            or.next_bit();
        }

        or.next();

        for _ in 0..5 {
            or.next_bit();
        }

        or.next();

        let result: Vec<u8> = or.collect();

        let expected_output = include_bytes!("../../test_inputs/tsz-compressed-data.bin");
        let mut expected_output_iter = expected_output.into_iter();

        expected_output_iter.next();
        expected_output_iter.next();
        expected_output_iter.next();
        expected_output_iter.next();

        let modified_expected_output: Vec<u8> = expected_output_iter.map(|v| *v).collect();

        assert_eq!(result, modified_expected_output);
    }
}

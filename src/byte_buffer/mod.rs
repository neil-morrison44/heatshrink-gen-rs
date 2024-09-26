pub struct ByteBuffer {
    index: usize,
    buffer: [bool; 16],
}

impl ByteBuffer {
    pub fn new() -> ByteBuffer {
        ByteBuffer {
            index: 0,
            buffer: [false; 16],
        }
    }

    pub fn add_byte(&mut self, new_byte: u8) -> Option<u8> {
        // Add the byte bit by bit to the buffer
        for i in 0..8 {
            self.buffer[self.index] = (new_byte & (1 << (7 - i))) != 0;
            self.index += 1;
        }

        return self.maybe_return_byte();
    }

    pub fn add_bit(&mut self, new_bit: bool) -> Option<u8> {
        self.buffer[self.index] = new_bit;
        self.index += 1;

        return self.maybe_return_byte();
    }

    pub fn last_byte(&mut self) -> Option<u8> {
        if self.index > 0 {
            self.add_byte(0)
        } else {
            None
        }
    }

    fn maybe_return_byte(&mut self) -> Option<u8> {
        if self.index >= 8 {
            // Construct the byte from the first 8 bits of the buffer
            let mut return_byte = 0u8;
            for i in 0..8 {
                if self.buffer[i] {
                    return_byte |= 1 << (7 - i);
                }
            }

            // Shift the remaining bits down by 8
            for i in 8..self.index {
                self.buffer[i - 8] = self.buffer[i];
            }

            // Update the index to reflect that 8 bits have been removed
            self.index -= 8;

            return Some(return_byte);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_bit_only() {
        let mut buffer = ByteBuffer::new();

        // Add 8 bits to fill the buffer and return a byte
        assert_eq!(buffer.add_bit(true), None); // 1
        assert_eq!(buffer.add_bit(false), None); // 10
        assert_eq!(buffer.add_bit(true), None); // 101
        assert_eq!(buffer.add_bit(true), None); // 1011
        assert_eq!(buffer.add_bit(false), None); // 10110
        assert_eq!(buffer.add_bit(false), None); // 101100
        assert_eq!(buffer.add_bit(true), None); // 1011001
        let result = buffer.add_bit(false);
        assert_eq!(result, Some(0b10110010), "{:08b}", result.unwrap()); // 10110010

        // No further bits added yet, so no new byte should be returned
        assert_eq!(buffer.add_bit(true), None); // 1...
    }

    #[test]
    fn test_add_byte_only() {
        let mut buffer = ByteBuffer::new();

        // Add a full byte, which should immediately return it
        assert_eq!(buffer.add_byte(0b10101010), Some(0b10101010));

        // Add another byte to check sequential behavior
        assert_eq!(buffer.add_byte(0b11110000), Some(0b11110000));
    }

    #[test]
    fn test_mix_bit_and_byte() {
        let mut buffer = ByteBuffer::new();

        // Add 1 bit, followed by a byte
        assert_eq!(buffer.add_bit(true), None); // 1

        let result = buffer.add_byte(0b01010101);
        assert_eq!(result, Some(0b10101010), "{:08b}", result.unwrap()); // 10101010 (1 from bit, 7 from byte)

        // Add more bits to verify the remaining byte (bits shifted down)
        assert_eq!(buffer.add_bit(true), None); // 11
        assert_eq!(buffer.add_bit(false), None); // 110
        assert_eq!(buffer.add_bit(true), None); // 1101
        assert_eq!(buffer.add_bit(true), None); // 11011
        assert_eq!(buffer.add_bit(false), None); // 110110
        assert_eq!(buffer.add_bit(true), None); // 1101101
        let result = buffer.add_bit(false);
        assert_eq!(result, Some(0b11011010), "{:08b}", result.unwrap()); // 11011010
    }
}

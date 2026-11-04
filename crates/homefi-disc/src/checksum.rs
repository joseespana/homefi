//! CRC-32 (IEEE) of the audio of a track: proves that what an encoder wrote decodes to what the
//! drive delivered, and goes in the rip log so two rips can be compared by eye.

const POLYNOMIAL: u32 = 0xEDB8_8320;
const TABLE_SIZE: usize = 256;

const fn table() -> [u32; TABLE_SIZE] {
    let mut table = [0u32; TABLE_SIZE];
    let mut i = 0;
    while i < TABLE_SIZE {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ POLYNOMIAL } else { crc >> 1 };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

static TABLE: [u32; TABLE_SIZE] = table();

/// Running CRC-32 and sample count of interleaved 16-bit audio (little endian bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmDigest {
    state: u32,
    /// Interleaved samples seen (both channels).
    pub samples: u64,
}

impl Default for PcmDigest {
    fn default() -> Self {
        PcmDigest { state: u32::MAX, samples: 0 }
    }
}

impl PcmDigest {
    pub fn push(&mut self, pcm: &[i16]) {
        let mut state = self.state;
        for sample in pcm {
            for byte in sample.to_le_bytes() {
                state = TABLE[usize::from((state as u8) ^ byte)] ^ (state >> 8);
            }
        }
        self.state = state;
        self.samples += pcm.len() as u64;
    }

    pub fn crc32(&self) -> u32 {
        !self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_standard_check_value() {
        // CRC-32 of the ASCII bytes "123456789" is 0xCBF43926; as i16 pairs (little endian).
        let bytes = b"12345678";
        let samples: Vec<i16> = bytes.chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        let mut digest = PcmDigest::default();
        digest.push(&samples);
        // 8 bytes: CRC-32("12345678") = 0x9AE0DAAF
        assert_eq!(digest.crc32(), 0x9AE0_DAAF);
        assert_eq!(digest.samples, 4);
    }

    #[test]
    fn pieces_equal_the_whole() {
        let pcm: Vec<i16> = (0..1000).map(|n| (n * 31 % 2000 - 1000) as i16).collect();
        let mut whole = PcmDigest::default();
        whole.push(&pcm);
        let mut parts = PcmDigest::default();
        for piece in pcm.chunks(77) {
            parts.push(piece);
        }
        assert_eq!(whole, parts);
    }
}

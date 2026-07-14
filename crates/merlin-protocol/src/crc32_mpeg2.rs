use crc::{CRC_32_MPEG_2, Crc};

const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_MPEG_2);

pub fn checksum(data: &[u8]) -> u32 {
    CRC32.checksum(data)
}

pub fn checksum_le(data: &[u8]) -> [u8; 4] {
    checksum(data).to_le_bytes()
}

pub fn checksum_parts(parts: &[&[u8]]) -> u32 {
    let mut digest = CRC32.digest();
    for part in parts {
        digest.update(part);
    }
    digest.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_check_value() {
        assert_eq!(checksum(b"123456789"), 0x0376_E6E7);
    }

    #[test]
    fn matches_known_set_date_suffixes() {
        let ts1: u32 = 1_783_201_515;
        let mut prefix1 = vec![0x08u8];
        prefix1.extend_from_slice(&ts1.to_le_bytes());
        assert_eq!(hex(&checksum_le(&prefix1)), "be75e2c2");

        let ts2: u32 = 1_783_201_588;
        let mut prefix2 = vec![0x08u8];
        prefix2.extend_from_slice(&ts2.to_le_bytes());
        assert_eq!(hex(&checksum_le(&prefix2)), "b33fb3a6");
    }

    #[test]
    fn checksum_parts_matches_the_concatenation() {
        let whole = b"0123456789abcdefghij";
        assert_eq!(checksum_parts(&[whole]), checksum(whole));
        assert_eq!(
            checksum_parts(&[&whole[..3], &whole[3..7], &whole[7..]]),
            checksum(whole)
        );
        assert_eq!(checksum_parts(&[b"", whole, b""]), checksum(whole));
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

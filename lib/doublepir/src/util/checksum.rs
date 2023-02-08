/// Simple XOR of all the u8's in a slice.
pub fn checksum_u8(data: &[u8]) -> u8 {
    let mut val = 0;
    for d in data {
        val ^= d;
    }
    val
}

/// Simple XOR of all the u32's in a slice.
pub fn checksum_u32(data: &[u32]) -> u32 {
    let mut val = 0;
    for d in data {
        val ^= d;
    }
    val
}

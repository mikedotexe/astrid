//! Small dependency-free SHA-256 implementation for deterministic local manifests.

const INITIAL: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

#[must_use]
pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    let bit_length = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    let mut padded = Vec::with_capacity(bytes.len().saturating_add(72));
    padded.extend_from_slice(bytes);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = INITIAL;
    for block in padded.chunks_exact(64) {
        let mut schedule = [0_u32; 64];
        for (index, word) in block.chunks_exact(4).take(16).enumerate() {
            schedule[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16_usize..64 {
            let previous_two = schedule[index.saturating_sub(2)];
            let previous_fifteen = schedule[index.saturating_sub(15)];
            let sigma_one = previous_two.rotate_right(17)
                ^ previous_two.rotate_right(19)
                ^ (previous_two >> 10);
            let sigma_zero = previous_fifteen.rotate_right(7)
                ^ previous_fifteen.rotate_right(18)
                ^ (previous_fifteen >> 3);
            schedule[index] = schedule[index.saturating_sub(16)]
                .wrapping_add(sigma_zero)
                .wrapping_add(schedule[index.saturating_sub(7)])
                .wrapping_add(sigma_one);
        }

        let [
            mut work_a,
            mut work_b,
            mut work_c,
            mut work_d,
            mut work_e,
            mut work_f,
            mut work_g,
            mut work_h,
        ] = state;
        for index in 0..64 {
            let choice = (work_e & work_f) ^ ((!work_e) & work_g);
            let majority = (work_a & work_b) ^ (work_a & work_c) ^ (work_b & work_c);
            let sum_one =
                work_e.rotate_right(6) ^ work_e.rotate_right(11) ^ work_e.rotate_right(25);
            let sum_zero =
                work_a.rotate_right(2) ^ work_a.rotate_right(13) ^ work_a.rotate_right(22);
            let temporary_one = work_h
                .wrapping_add(sum_one)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(schedule[index]);
            let temporary_two = sum_zero.wrapping_add(majority);
            work_h = work_g;
            work_g = work_f;
            work_f = work_e;
            work_e = work_d.wrapping_add(temporary_one);
            work_d = work_c;
            work_c = work_b;
            work_b = work_a;
            work_a = temporary_one.wrapping_add(temporary_two);
        }
        state[0] = state[0].wrapping_add(work_a);
        state[1] = state[1].wrapping_add(work_b);
        state[2] = state[2].wrapping_add(work_c);
        state[3] = state[3].wrapping_add(work_d);
        state[4] = state[4].wrapping_add(work_e);
        state[5] = state[5].wrapping_add(work_f);
        state[6] = state[6].wrapping_add(work_g);
        state[7] = state[7].wrapping_add(work_h);
    }

    let mut output = [0_u8; 32];
    for (index, word) in state.iter().enumerate() {
        let start = index.saturating_mul(4);
        let end = start.saturating_add(4);
        output[start..end].copy_from_slice(&word.to_be_bytes());
    }
    output
}

#[must_use]
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in sha256(bytes) {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub(crate) fn validate_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub(crate) fn validate_signature_hex(value: &str) -> bool {
    value.len() == 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

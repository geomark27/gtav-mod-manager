//! Deterministic fixture data and the base file lists of valid installations.

use gmm_domain::Edition;

/// Deterministic pseudo-random bytes derived from `seed` (xorshift64* seeded with the FNV-1a
/// hash of the seed bytes). Same seed and length give the same bytes on every platform.
pub fn fake_bytes(seed: &str, len: usize) -> Vec<u8> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let mut state = if h == 0 { 0x9e37_79b9_7f4a_7c15 } else { h };
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let word = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        for b in word.to_le_bytes() {
            if out.len() < len {
                out.push(b);
            }
        }
    }
    out
}

/// Base files of a structurally valid Legacy installation.
/// ASSUMPTION A-05 (docs/roadmap.md): verify on real installs in M2/M7.
pub const LEGACY_BASE_FILES: &[&str] = &[
    "GTA5.exe",
    "PlayGTAV.exe",
    "common.rpf",
    "x64a.rpf",
    "update/update.rpf",
    "bink2w64.dll",
];

/// Base files of a structurally valid Enhanced installation. ASSUMPTION A-05.
pub const ENHANCED_BASE_FILES: &[&str] = &[
    "GTA5_Enhanced.exe",
    "PlayGTAV.exe",
    "common.rpf",
    "x64a.rpf",
    "update/update.rpf",
    "bink2w64.dll",
];

/// Base files of a structurally valid Red Dead Redemption 2 installation. ASSUMPTION A-11.
pub const RDR2_BASE_FILES: &[&str] = &["RDR2.exe", "PlayRDR2.exe", "common_0.rpf", "bink2w64.dll"];

/// Base files of the given edition.
pub fn base_files(edition: Edition) -> &'static [&'static str] {
    match edition {
        Edition::Legacy => LEGACY_BASE_FILES,
        Edition::Enhanced => ENHANCED_BASE_FILES,
        Edition::Rdr2 => RDR2_BASE_FILES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_exact_length() {
        assert_eq!(fake_bytes("a", 100), fake_bytes("a", 100));
        for len in [0, 1, 7, 8, 9, 4096] {
            assert_eq!(fake_bytes("a", len).len(), len);
        }
        assert_eq!(fake_bytes("a", 20)[..10], fake_bytes("a", 10)[..]);
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(fake_bytes("a", 64), fake_bytes("b", 64));
    }

    #[test]
    fn pinned_prefix() {
        assert_eq!(
            fake_bytes("GTA5.exe", 8),
            vec![21, 81, 173, 59, 49, 169, 159, 29]
        );
    }

    #[test]
    fn base_file_lists() {
        assert_eq!(base_files(Edition::Legacy), LEGACY_BASE_FILES);
        assert_eq!(base_files(Edition::Enhanced), ENHANCED_BASE_FILES);
        assert_eq!(base_files(Edition::Rdr2), RDR2_BASE_FILES);
    }
}

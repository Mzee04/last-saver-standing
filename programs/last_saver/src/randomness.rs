use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;

use crate::error::LastSaverError;

/// ORAO VRF classic program. Same address on devnet and mainnet.
/// https://github.com/orao-network/solana-vrf
pub const ORAO_VRF_ID: Pubkey = pubkey!("VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y");

/// PDA seed used by ORAO for request accounts (`randomness_account_address`).
pub const ORAO_REQUEST_SEED: &[u8] = b"orao-vrf-randomness-request";

/// `sha256("account:RandomnessV2")[..8]`, Anchor's account discriminator.
pub const ORAO_V2_DISC: [u8; 8] = [139, 239, 184, 215, 227, 86, 191, 226];

#[derive(Clone, Copy, Debug)]
pub struct OraoView {
    pub seed: [u8; 32],
    /// `None` while the request is still pending.
    pub randomness: Option<[u8; 64]>,
}

/// Parse an ORAO `RandomnessV2` account.
///
/// Layout (borsh, after the 8-byte Anchor discriminator):
/// - variant `0` Pending: client pubkey, seed, vec of responses
/// - variant `1` Fulfilled: client pubkey, seed, 64-byte randomness
///
/// Both variants store `client` then `seed` at the same offset, which is
/// the layout in `orao_solana_vrf` 0.8.x (`RequestAccount`).
pub fn parse_orao(data: &[u8]) -> Result<OraoView> {
    require!(data.len() >= 8 + 1 + 32 + 32, LastSaverError::BadVrf);
    require!(data[..8] == ORAO_V2_DISC, LastSaverError::BadVrf);
    let seed_at = 8 + 1 + 32;
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&data[seed_at..seed_at + 32]);
    match data[8] {
        0 => Ok(OraoView {
            seed,
            randomness: None,
        }),
        1 => {
            let rand_at = seed_at + 32;
            require!(data.len() >= rand_at + 64, LastSaverError::BadVrf);
            let mut randomness = [0u8; 64];
            randomness.copy_from_slice(&data[rand_at..rand_at + 64]);
            require!(randomness != [0u8; 64], LastSaverError::VrfPending);
            Ok(OraoView {
                seed,
                randomness: Some(randomness),
            })
        }
        _ => err!(LastSaverError::BadVrf),
    }
}

pub fn assert_orao_pda(seed: &[u8; 32], account: &AccountInfo) -> Result<()> {
    let (expected, _) =
        Pubkey::find_program_address(&[ORAO_REQUEST_SEED, seed.as_ref()], &ORAO_VRF_ID);
    require_keys_eq!(account.key(), expected, LastSaverError::BadVrf);
    require_keys_eq!(*account.owner, ORAO_VRF_ID, LastSaverError::BadVrf);
    Ok(())
}

/// Map 64 bytes of randomness into `[min_gap, max_gap]` seconds, inclusive.
pub fn gap_seconds(randomness: &[u8; 64], min_gap: i64, max_gap: i64) -> i64 {
    if max_gap <= min_gap {
        return min_gap;
    }
    let span = (max_gap - min_gap) as u64;
    let sample = u64::from_le_bytes(randomness[..8].try_into().unwrap());
    min_gap + (sample % (span + 1)) as i64
}

/// Demo-mode randomness: sha256(slot_hash || seed || window_id), expanded to 64 bytes.
///
/// The slot hash is unknown at commit time, so the crank cannot grind the gap.
/// This is commit-reveal against Solana's slot-hash sysvar, not a VRF. Leaders
/// can influence future slot hashes. Production rounds should use [`RandomnessMode::Orao`].
pub fn demo_randomness(slot_hash: &[u8; 32], seed: &[u8; 32], window_id: u16) -> [u8; 64] {
    let mut buf = [0u8; 66];
    buf[..32].copy_from_slice(slot_hash);
    buf[32..64].copy_from_slice(seed);
    buf[64..66].copy_from_slice(&window_id.to_le_bytes());
    let first = hash(&buf).to_bytes();
    let mut domain = first;
    domain[0] ^= 0x5a;
    let second = hash(&domain).to_bytes();
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&first);
    out[32..].copy_from_slice(&second);
    out
}

/// SlotHashes sysvar: `u64` count, then `count` entries of `(u64 slot, [u8; 32] hash)`.
pub fn read_slot_hash(data: &[u8], slot: u64) -> Option<[u8; 32]> {
    if data.len() < 8 {
        return None;
    }
    let count = u64::from_le_bytes(data[..8].try_into().ok()?) as usize;
    if count > 512 {
        return None;
    }
    let mut offset = 8usize;
    for _ in 0..count {
        if offset + 40 > data.len() {
            return None;
        }
        let entry_slot = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        if entry_slot == slot {
            let mut hash_bytes = [0u8; 32];
            hash_bytes.copy_from_slice(&data[offset + 8..offset + 40]);
            return Some(hash_bytes);
        }
        offset += 40;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_stays_inside_bounds() {
        let mut randomness = [0u8; 64];
        assert_eq!(gap_seconds(&randomness, 5, 5), 5);
        randomness[..8].copy_from_slice(&u64::MAX.to_le_bytes());
        let gap = gap_seconds(&randomness, 10, 20);
        assert!((10..=20).contains(&gap));
        randomness[..8].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(gap_seconds(&randomness, 10, 20), 10);
    }

    #[test]
    fn parses_pending_and_fulfilled() {
        let client = Pubkey::new_unique();
        let seed = [7u8; 32];
        let mut pending = vec![0u8; 8 + 1 + 32 + 32 + 4];
        pending[..8].copy_from_slice(&ORAO_V2_DISC);
        pending[8] = 0;
        pending[9..41].copy_from_slice(client.as_ref());
        pending[41..73].copy_from_slice(&seed);
        let view = parse_orao(&pending).unwrap();
        assert_eq!(view.seed, seed);
        assert!(view.randomness.is_none());

        let mut randomness = [0u8; 64];
        randomness[0] = 9;
        let mut full = Vec::new();
        full.extend_from_slice(&ORAO_V2_DISC);
        full.push(1);
        full.extend_from_slice(client.as_ref());
        full.extend_from_slice(&seed);
        full.extend_from_slice(&randomness);
        let view = parse_orao(&full).unwrap();
        assert_eq!(view.randomness, Some(randomness));

        full[8] = 0;
        // Pending variant ignores trailing bytes but a zero variant with the
        // fulfilled buffer is still pending.
        assert!(parse_orao(&full).unwrap().randomness.is_none());
        full[0] ^= 0xff;
        assert!(parse_orao(&full).is_err());
    }
}

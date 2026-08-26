/// Pinned integer mixer used for independent deterministic bot choices.
#[must_use]
pub(super) fn sample_u64(seed: u64, stream: u64, tick: u64) -> u64 {
    let mut value = seed
        ^ stream.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ tick.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[must_use]
pub(super) fn signed_unit(seed: u64, stream: u64, tick: u64) -> f32 {
    let sample = sample_u64(seed, stream, tick) >> 48;
    let sample = u16::try_from(sample).expect("the sampler retains exactly 16 bits");
    let unit = f32::from(sample) / f32::from(u16::MAX);
    unit.mul_add(2.0, -1.0)
}

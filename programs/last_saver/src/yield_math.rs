/// Seconds used by the simulated APY. Not a leap-year average: 365 * 86400.
pub const SECONDS_PER_YEAR: u128 = 31_536_000;

/// Simulated yield on `principal` at `annual_bps` for `seconds`.
///
/// This is a straight-line quote, not interest taken from principal:
/// `principal * bps * seconds / (10_000 * SECONDS_PER_YEAR)`.
/// The program pays the result from the sponsor reserve, and only up to
/// the reserve's liquid balance.
pub fn simulated_yield(principal: u64, annual_bps: u16, seconds: u64) -> u64 {
    if principal == 0 || annual_bps == 0 || seconds == 0 {
        return 0;
    }
    let numerator = (principal as u128)
        .saturating_mul(annual_bps as u128)
        .saturating_mul(seconds as u128);
    let denominator = 10_000u128 * SECONDS_PER_YEAR;
    (numerator / denominator) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_year_at_five_percent_is_exact() {
        let principal = 3_000_000_000u64;
        let yield_lamports = simulated_yield(principal, 500, SECONDS_PER_YEAR as u64);
        assert_eq!(yield_lamports, 150_000_000);
    }

    #[test]
    fn zero_inputs_pay_nothing() {
        assert_eq!(simulated_yield(0, 500, 1_000), 0);
        assert_eq!(simulated_yield(1_000, 0, 1_000), 0);
        assert_eq!(simulated_yield(1_000, 500, 0), 0);
    }

    #[test]
    fn halves_scale() {
        let year = simulated_yield(2_000_000_000, 500, SECONDS_PER_YEAR as u64);
        let half = simulated_yield(2_000_000_000, 500, (SECONDS_PER_YEAR / 2) as u64);
        assert_eq!(year, 100_000_000);
        assert_eq!(half, 50_000_000);
    }
}

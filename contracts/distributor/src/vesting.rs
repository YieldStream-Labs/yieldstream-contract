use crate::errors::ContractError;
use crate::types::VestingSchedule;

/// Computes the linearly vested portion of `amount` at `timestamp`.
///
/// Before the cliff nothing is vested, and at or after `end_time` the full
/// amount is vested. Between those points vesting is linear from `start_time`.
/// Integer arithmetic is used exclusively, rounding down to the nearest unit.
pub fn vested_amount(
    schedule: &VestingSchedule,
    amount: i128,
    timestamp: u64,
) -> Result<i128, ContractError> {
    if amount <= 0 {
        return Ok(0);
    }
    if timestamp < schedule.cliff_time {
        return Ok(0);
    }
    if timestamp >= schedule.end_time {
        return Ok(amount);
    }

    let duration = schedule
        .end_time
        .checked_sub(schedule.start_time)
        .ok_or(ContractError::InvalidVestingSchedule)?;
    if duration == 0 {
        return Ok(amount);
    }

    let elapsed = timestamp.saturating_sub(schedule.start_time);
    if elapsed >= duration {
        return Ok(amount);
    }

    let product = amount
        .checked_mul(i128::from(elapsed))
        .ok_or(ContractError::ArithmeticOverflow)?;
    Ok(product / i128::from(duration))
}

use std::collections::HashMap;

use cosmwasm_std::Uint128;

use crate::models::{Claim, Drawing, Payout};

pub fn hash_numbers(numbers: &Vec<u16>) -> String {
  let parts: Vec<String> = numbers.iter().map(|n| n.to_string()).collect();
  parts.join(":")
}

pub fn mul_pct(
  total: Uint128,
  pct: Uint128,
) -> Uint128 {
  total.multiply_ratio(pct, Uint128::from(1_000_000u128))
}

pub fn calc_total_claim_amount(
  claim: &Claim,
  drawing: &Drawing,
  payouts: &HashMap<u8, Payout>,
) -> Uint128 {
  let mut claim_amount = Uint128::zero();
  let pot_size = drawing.get_pot_size();
  for (match_count, n_tickets) in claim.matches.iter().enumerate().skip(1) {
    if let Some(payout) = payouts.get(&(match_count as u8)) {
      let n_total_tickets = drawing.match_counts[match_count] as u32;
      if n_total_tickets > 0 {
        // Add incentive owed to user
        claim_amount += payout.incentive * Uint128::from(*n_tickets);
        // Add portion of pot owed to user
        claim_amount += mul_pct(pot_size, payout.pct)
          .multiply_ratio((*n_tickets) as u128, n_total_tickets as u128)
      }
    }
  }
  claim_amount
}

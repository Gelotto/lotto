use std::collections::HashMap;

use cosmwasm_std::Uint128;

use crate::models::{Claim, Drawing, Payout};

const ONE_MILLION: u128 = 1_000_000;

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
  for (n_matching_numbers, n_claim_tickets) in claim.match_counts.iter().enumerate().skip(1) {
    if let Some(payout) = payouts.get(&(n_matching_numbers as u8)) {
      let n_total_tickets = drawing.match_counts[n_matching_numbers] as u32;

      // Add incentive owed to user
      claim_amount += payout.incentive;

      // Add portion of pot owed to user
      claim_amount += mul_pct(drawing.total_balance, payout.pct).multiply_ratio(
        (*n_claim_tickets) as u128 * ONE_MILLION,
        (n_total_tickets as u128) * ONE_MILLION,
      );
    }
  }
  claim_amount
}

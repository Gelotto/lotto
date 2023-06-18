use crate::error::ContractError;
use crate::msg::ClaimResponse;
use crate::state::{load_claim, load_drawing, load_payouts, ROUND_NO, WINNING_TICKETS};
use crate::util::calc_total_claim_amount;
use cosmwasm_std::{Addr, Deps, Order};

pub fn claim(
  deps: Deps,
  address: Addr,
) -> Result<Option<ClaimResponse>, ContractError> {
  if let Ok(claim) = load_claim(deps.storage, &address) {
    if let Ok(drawing) = load_drawing(deps.storage, claim.round_no) {
      let round_no = ROUND_NO.load(deps.storage)?;
      let payouts = load_payouts(deps.storage)?;
      let claim_amount = calc_total_claim_amount(&claim, &drawing, &payouts);
      let winning_tickets: Vec<Vec<u16>> = WINNING_TICKETS
        .prefix((round_no.into(), address.clone()))
        .range(deps.storage, None, None, Order::Ascending)
        .map(|r| r.unwrap().1)
        .collect();

      return Ok(Some(ClaimResponse {
        round_no: claim.round_no,
        amount: claim_amount,
        winning_tickets,
      }));
    }
  }

  Ok(None)
}

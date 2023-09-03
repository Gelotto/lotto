use cosmwasm_std::{Addr, Deps, Order};

use crate::{
  error::ContractError,
  msg::ClaimView,
  state::{load_drawing, load_payouts, CLAIMS, JACKPOT_CLAIMANTS},
  util::calc_total_claim_amount,
};

pub fn claims_pending_approval(deps: Deps) -> Result<Vec<ClaimView>, ContractError> {
  let addrs: Vec<Addr> = JACKPOT_CLAIMANTS
    .keys(deps.storage, None, None, Order::Ascending)
    .map(|r| r.unwrap())
    .collect();

  let mut claims: Vec<ClaimView> = Vec::with_capacity(addrs.len());
  let payouts = load_payouts(deps.storage)?;
  for addr in addrs.iter() {
    let mut claim = CLAIMS.load(deps.storage, addr.clone())?;
    let drawing = load_drawing(deps.storage, claim.round_no)?;
    claim.amount = Some(calc_total_claim_amount(&claim, &drawing, &payouts));
    claims.push(ClaimView {
      owner: addr.clone(),
      amount: claim.amount,
      is_approved: claim.is_approved,
      matches: claim.matches,
      round_no: claim.round_no,
      tickets: claim.tickets,
    });
  }

  Ok(claims)
}

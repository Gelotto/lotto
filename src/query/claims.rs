use std::{collections::HashMap, marker::PhantomData};

use cosmwasm_std::{Addr, Deps, Order};
use cw_storage_plus::Bound;

use crate::{
  error::ContractError,
  models::{Claim, Drawing},
  state::{load_drawing, load_payouts, CLAIMS},
  util::calc_total_claim_amount,
};

pub const MAX_LIMIT: u8 = 50;

pub fn claims(
  deps: Deps,
  maybe_cursor: Option<Addr>,
  maybe_limit: Option<u8>,
) -> Result<Vec<Claim>, ContractError> {
  let limit = maybe_limit.unwrap_or(MAX_LIMIT) as usize;

  let range_min = maybe_cursor
    .and_then(|addr| Some(Bound::Exclusive((addr.clone(), PhantomData))))
    .or(None);

  let mut claims: Vec<Claim> = CLAIMS
    .range(deps.storage, range_min, None, Order::Ascending)
    .take(limit)
    .map(|result| result.unwrap().1)
    .collect();

  let payouts = &load_payouts(deps.storage)?;
  let mut drawings: HashMap<u64, Drawing> = HashMap::with_capacity(4);

  for claim in claims.iter_mut() {
    // Get the Drawing corresponding to the Claim. first check in-memory
    // drawings cache; otherwise, read from storage.
    let drawing = match drawings.get(&claim.round_no.into()) {
      Some(drawing) => drawing,
      None => {
        drawings.insert(
          claim.round_no.into(),
          load_drawing(deps.storage, claim.round_no)?,
        );
        drawings.get(&claim.round_no.into()).unwrap()
      },
    };
    // compute and set the claim amount.
    claim.amount = Some(calc_total_claim_amount(&claim, &drawing, payouts))
  }

  Ok(claims)
}

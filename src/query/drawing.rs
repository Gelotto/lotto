use crate::error::ContractError;
use crate::models::Drawing;
use crate::state::DRAWINGS;
use cosmwasm_std::{Deps, Uint64};

pub fn drawing(
  deps: Deps,
  round_no: Uint64,
) -> Result<Option<Drawing>, ContractError> {
  Ok(DRAWINGS.may_load(deps.storage, round_no.u64())?)
}

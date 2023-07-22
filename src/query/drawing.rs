use crate::models::Drawing;
use crate::state::DRAWINGS;
use crate::{error::ContractError, state::ROUND_NO};
use cosmwasm_std::{Deps, Uint64};

pub fn drawing(
  deps: Deps,
  maybe_round_no: Option<Uint64>,
) -> Result<Option<Drawing>, ContractError> {
  let round_no = maybe_round_no.unwrap_or_else(|| {
    let round_no = ROUND_NO.load(deps.storage).unwrap();
    if round_no == Uint64::one() {
      round_no
    } else {
      round_no - Uint64::one()
    }
  });

  let maybe_drawing = DRAWINGS.may_load(deps.storage, round_no.u64())?;
  if let Some(mut drawing) = maybe_drawing {
    drawing.round_no = Some(round_no);
    return Ok(Some(drawing));
  }

  Ok(None)
}

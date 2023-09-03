use crate::{
  error::ContractError,
  state::{load_claim, process_claim, require_active_game_state, CONFIG_USE_APPROVAL},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn claim(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> Result<Response, ContractError> {
  require_active_game_state(deps.storage)?;
  let resp = Response::new().add_attributes(vec![attr("action", "claim")]);
  let claim = load_claim(deps.storage, &info.sender)?;

  // If the claim is for a jackpot, abort if pending admin approval
  if CONFIG_USE_APPROVAL.load(deps.storage)? {
    if let Some(jackpot_match_count) = claim.matches.last() {
      if *jackpot_match_count > 0 && !claim.is_approved {
        return Err(ContractError::PendingApproval);
      }
    }
  }

  Ok(
    if let Some(transfer_submsg) = process_claim(deps.storage, &info.sender, claim, false)? {
      resp.add_submessage(transfer_submsg)
    } else {
      resp
    },
  )
}

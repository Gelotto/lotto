use crate::{
  error::ContractError,
  state::{process_claim, require_active_game_state},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn claim(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> Result<Response, ContractError> {
  require_active_game_state(deps.storage)?;
  let resp = Response::new().add_attributes(vec![attr("action", "claim")]);
  Ok(
    if let Some(transfer_submsg) = process_claim(deps.storage, &info.sender)? {
      resp.add_submessage(transfer_submsg)
    } else {
      resp
    },
  )
}

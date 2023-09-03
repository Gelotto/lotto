use crate::{
  error::ContractError,
  state::{ensure_sender_is_allowed, load_claim, process_claim},
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn reject(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  claimant_addr: Addr,
) -> Result<Response, ContractError> {
  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "reject")?;

  let claim = load_claim(deps.storage, &claimant_addr)?;
  let is_rejected = true;

  process_claim(deps.storage, &info.sender, claim, is_rejected)?;

  Ok(Response::new().add_attributes(vec![attr("action", "reject")]))
}

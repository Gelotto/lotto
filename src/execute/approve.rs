use crate::{
  error::ContractError,
  state::{ensure_sender_is_allowed, CLAIMS},
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn approve(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  address: Addr,
) -> Result<Response, ContractError> {
  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "approve")?;

  CLAIMS.update(
    deps.storage,
    address.clone(),
    |maybe_claim| -> Result<_, ContractError> {
      if let Some(mut claim) = maybe_claim {
        if !claim.is_approved {
          claim.is_approved = true;
          return Ok(claim);
        } else {
          return Err(ContractError::NotAuthorized);
        }
      }
      Err(ContractError::ClaimNotFound)
    },
  )?;

  Ok(Response::new().add_attributes(vec![attr("action", "approve")]))
}

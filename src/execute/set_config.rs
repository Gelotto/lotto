use crate::{
  error::ContractError,
  models::Config,
  state::{ensure_sender_is_allowed, STAGED_CONFIG},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn set_config(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  config: Config,
) -> Result<Response, ContractError> {
  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "set_config")?;
  config.validate(deps.api)?;

  STAGED_CONFIG.save(deps.storage, &Some(config))?;

  Ok(Response::new().add_attributes(vec![attr("action", "set_config")]))
}

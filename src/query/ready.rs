use crate::{error::ContractError, state::is_ready};
use cosmwasm_std::{Deps, Env};

pub fn ready(
  deps: Deps,
  env: Env,
) -> Result<bool, ContractError> {
  Ok(is_ready(deps.storage, &env.block)?)
}

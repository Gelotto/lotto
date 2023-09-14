use crate::{
  error::ContractError,
  models::Config,
  state::{
    ensure_sender_is_allowed, require_active_game_state, CONFIG_HOUSE_ADDR, CONFIG_MARKETING,
    CONFIG_NOIS_PROXY, CONFIG_STYLE, STAGED_CONFIG,
  },
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn set_config(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  config: Config,
) -> Result<Response, ContractError> {
  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "set_config")?;
  require_active_game_state(deps.storage)?;

  config.validate(deps.api)?;

  CONFIG_MARKETING.save(deps.storage, &config.marketing)?;
  CONFIG_HOUSE_ADDR.save(deps.storage, &config.house_address)?;
  CONFIG_STYLE.save(deps.storage, &config.style)?;
  CONFIG_NOIS_PROXY.save(deps.storage, &config.nois_proxy)?;

  STAGED_CONFIG.save(deps.storage, &Some(config))?;

  Ok(Response::new().add_attributes(vec![attr("action", "set_config")]))
}

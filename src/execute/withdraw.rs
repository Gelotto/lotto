use crate::{
  error::ContractError,
  state::{ensure_sender_is_allowed, require_active_game_state, CONFIG_TOKEN},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};
use cw_lib::utils::funds::{build_send_submsg, get_token_balance};

pub fn withdraw(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
) -> Result<Response, ContractError> {
  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "withdraw")?;
  require_active_game_state(deps.storage)?;

  let token = CONFIG_TOKEN.load(deps.storage)?;
  let contract_balance = get_token_balance(deps.querier, &env.contract.address, &token)?;
  let resp = Response::new().add_attributes(vec![attr("action", "withdraw")]);

  Ok(if contract_balance.is_zero() {
    resp
  } else {
    resp.add_submessage(build_send_submsg(&info.sender, contract_balance, &token)?)
  })
}

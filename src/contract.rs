use crate::error::ContractError;
use crate::execute;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::query;
use crate::state::{self, STAGED_CONFIG};
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;

const CONTRACT_NAME: &str = "crates.io:lotto";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  msg: InstantiateMsg,
) -> Result<Response, ContractError> {
  set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
  state::initialize(deps, &env, &info, &msg)?;
  Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  msg: ExecuteMsg,
) -> Result<Response, ContractError> {
  match msg {
    ExecuteMsg::Buy { player, tickets } => execute::buy(deps, env, info, player, tickets),
    ExecuteMsg::BuySeed {
      player,
      count,
      seed,
    } => execute::buy_seed(deps, env, info, player, count, seed),
    ExecuteMsg::Draw { entropy } => execute::draw(deps, env, info, entropy),
    ExecuteMsg::Claim {} => execute::claim(deps, env, info),
    ExecuteMsg::Withdraw {} => execute::withdraw(deps, env, info),
    ExecuteMsg::SetConfig { config } => execute::set_config(deps, env, info, config),
  }
}

#[entry_point]
pub fn query(
  deps: Deps,
  env: Env,
  msg: QueryMsg,
) -> Result<Binary, ContractError> {
  let result = match msg {
    QueryMsg::Select { fields, wallet } => to_binary(&query::select(deps, env, fields, wallet)?),
    QueryMsg::Drawing { round_no } => to_binary(&query::drawing(deps, round_no)?),
    QueryMsg::Ready => to_binary(&query::ready(deps, env)?),
    QueryMsg::Claims { cursor, limit } => to_binary(&query::claims(deps, cursor, limit)?),
  }?;
  Ok(result)
}

#[entry_point]
pub fn migrate(
  deps: DepsMut,
  _env: Env,
  msg: MigrateMsg,
) -> Result<Response, ContractError> {
  set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
  match msg {
    MigrateMsg::V0_0_4 {} => {
      if STAGED_CONFIG.load(deps.storage).is_err() {
        STAGED_CONFIG.save(deps.storage, &None)?;
      }
    },
    MigrateMsg::NoOp {} => {},
  }
  Ok(Response::default())
}

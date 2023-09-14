use crate::error::ContractError;
use crate::execute;
use crate::models::RoundStatus;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::query;
use crate::state::{self, CONFIG_USE_APPROVAL, PREV_HEIGHT, ROUND_STATUS};
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
    ExecuteMsg::Claim {} => execute::claim(deps, env, info),
    ExecuteMsg::Withdraw {} => execute::withdraw(deps, env, info),
    ExecuteMsg::SetConfig { config } => execute::set_config(deps, env, info, config),
    ExecuteMsg::Approve { address } => execute::approve(deps, env, info, address),
    ExecuteMsg::Reject { address } => execute::reject(deps, env, info, address),
    ExecuteMsg::Draw {} => execute::draw(deps, env, info, None),
    ExecuteMsg::NoisReceive { callback } => execute::draw(deps, env, info, Some(callback)),
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
    QueryMsg::ClaimsPendingApproval {} => to_binary(&query::claims_pending_approval(deps)?),
  }?;
  Ok(result)
}

#[entry_point]
pub fn migrate(
  deps: DepsMut,
  env: Env,
  msg: MigrateMsg,
) -> Result<Response, ContractError> {
  set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
  match msg {
    MigrateMsg::NoOp {} => {},
    MigrateMsg::V0_0_9 {} => {
      PREV_HEIGHT.save(deps.storage, &env.block.height.into())?;
      CONFIG_USE_APPROVAL.save(deps.storage, &true)?;
    },
    MigrateMsg::V0_1_0 { set_active } => {
      if set_active {
        ROUND_STATUS.save(deps.storage, &RoundStatus::Active)?;
      }
    },
  }
  Ok(Response::default())
}

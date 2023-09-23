use std::collections::HashMap;

use crate::error::ContractError;
use crate::execute;
use crate::models::{Claim, ClaimV1};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::query;
use crate::state::{self, CLAIMS};
use cosmwasm_std::{entry_point, Addr, Order};
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw_storage_plus::Map;

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
    ExecuteMsg::Buy {
      player,
      referrer,
      tickets,
    } => execute::buy(deps, env, info, player, referrer, tickets),
    ExecuteMsg::BuySeed {
      player,
      referrer,
      count,
      seed,
    } => execute::buy_seed(deps, env, info, player, referrer, count, seed),
    ExecuteMsg::SenderBuySeed {
      referrer,
      count,
      seed,
    } => execute::sender_buy_seed(deps, env, info, referrer, count, seed),
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
  _env: Env,
  msg: MigrateMsg,
) -> Result<Response, ContractError> {
  set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
  match msg {
    MigrateMsg::NoOp {} => {},
    MigrateMsg::V0_1_1 {} => {
      // Migrate stale claim records leftover from the past...
      let claims_v1_map: Map<Addr, ClaimV1> = Map::new("claims");
      let mut v1_claims: HashMap<Addr, ClaimV1> = HashMap::with_capacity(2);

      for maybe_entry in claims_v1_map.range(deps.storage, None, None, Order::Ascending) {
        if let Ok((addr, claim)) = maybe_entry {
          v1_claims.insert(addr, claim);
        }
      }

      for (addr, claim_v1) in v1_claims.iter() {
        claims_v1_map.remove(deps.storage, addr.clone());
        CLAIMS.save(
          deps.storage,
          addr.clone(),
          &Claim {
            amount: claim_v1.amount,
            is_approved: true,
            round_no: claim_v1.round_no,
            matches: claim_v1.matches.to_owned(),
            tickets: claim_v1.tickets.to_owned(),
          },
        )?;
      }
    },
  }
  Ok(Response::default())
}

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Uint64};
use cw_lib::models::Owner;

use crate::models::{Config, PlayerAccount};

#[cw_serde]
pub struct InstantiateMsg {
  pub owner: Option<Owner>,
  pub config: Config,
}

#[cw_serde]
pub enum ExecuteMsg {
  Buy { tickets: Vec<Vec<u16>> },
  Draw {},
}

#[cw_serde]
pub enum QueryMsg {
  Select {
    fields: Option<Vec<String>>,
    wallet: Option<Addr>,
  },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct SelectResponse {
  pub owner: Option<Owner>,
  pub config: Option<Config>,
  pub round_count: Option<Uint64>,
  pub tax_rate: Option<Uint128>,
  pub account: Option<PlayerAccount>,
}

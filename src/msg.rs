use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Uint64};
use cw_lib::models::Owner;

use crate::models::{AccountTotals, Config, Round};

#[cw_serde]
pub struct InstantiateMsg {
  pub owner: Option<Owner>,
  pub config: Config,
  pub winning_numbers: Option<Vec<u16>>,
}

#[cw_serde]
pub enum ExecuteMsg {
  Buy { tickets: Vec<Vec<u16>> },
  Draw {},
}

#[cw_serde]
pub enum QueryMsg {
  Drawing {
    round_no: Uint64,
  },
  Select {
    fields: Option<Vec<String>>,
    wallet: Option<Addr>,
  },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct ClaimView {
  pub round_no: Uint64,
  pub amount: Uint128,
  pub winning_tickets: Vec<Vec<u16>>,
}

#[cw_serde]
pub struct AccountView {
  pub totals: AccountTotals,
  pub tickets: Vec<Vec<u16>>,
  pub claim: Option<ClaimView>,
}

#[cw_serde]
pub struct SelectResponse {
  pub owner: Option<Owner>,
  pub config: Option<Config>,
  pub round: Option<Round>,
  pub tax_rate: Option<Uint128>,
  pub balance: Option<Uint128>,
  pub account: Option<AccountView>,
}

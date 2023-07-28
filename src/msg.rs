use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Uint64};
use cw_lib::models::Owner;

use crate::models::{AccountTotals, Claim, Config, Round, Ticket};

#[cw_serde]
pub struct InstantiateMsg {
  pub owner: Option<Owner>,
  pub config: Config,
  pub winning_numbers: Option<Vec<u16>>,
}

#[cw_serde]
pub enum ExecuteMsg {
  SetConfig { config: Config },
  Buy { tickets: Vec<Vec<u16>> },
  Draw {},
  Claim {},
  Withdraw {},
}

#[cw_serde]
pub enum QueryMsg {
  Ready,
  Drawing {
    round_no: Option<Uint64>,
  },
  Select {
    fields: Option<Vec<String>>,
    wallet: Option<Addr>,
  },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct AccountView {
  pub totals: AccountTotals,
  pub tickets: Vec<Ticket>,
  pub claim: Option<Claim>,
}

#[cw_serde]
pub struct SelectResponse {
  pub owner: Option<Owner>,
  pub config: Option<Config>,
  pub round: Option<Round>,
  pub tax_rate: Option<Uint128>,
  pub balance_claimable: Option<Uint128>,
  pub balance: Option<Uint128>,
  pub account: Option<AccountView>,
}

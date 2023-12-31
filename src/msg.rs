use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Uint64};
use cw_lib::models::Owner;
use nois::NoisCallback;

use crate::models::{AccountTotals, Claim, Config, Round, Ticket};

#[cw_serde]
pub struct InstantiateMsg {
  pub owner: Option<Owner>,
  pub config: Config,
  pub winning_numbers: Option<Vec<u16>>,
}

#[cw_serde]
pub enum ExecuteMsg {
  SetConfig {
    config: Config,
  },
  Buy {
    player: Option<Addr>,
    referrer: Option<Addr>,
    tickets: Vec<Vec<u16>>,
  },
  BuySeed {
    player: Option<Addr>,
    referrer: Option<Addr>,
    count: u16,
    seed: u32,
  },
  SenderBuySeed {
    referrer: Option<Addr>,
    count: u16,
    seed: u32,
  },
  Draw {},
  Claim {},
  Withdraw {},
  Approve {
    address: Addr,
  },
  Reject {
    address: Addr,
  },
  NoisReceive {
    callback: NoisCallback,
  },
}

#[cw_serde]
pub enum QueryMsg {
  Ready,
  Drawing {
    round_no: Option<Uint64>,
  },
  Claims {
    cursor: Option<Addr>,
    limit: Option<u8>,
  },
  Select {
    fields: Option<Vec<String>>,
    wallet: Option<Addr>,
  },
  ClaimsPendingApproval {},
}

#[cw_serde]
pub enum MigrateMsg {
  V0_1_1 {},
  NoOp {},
}

#[cw_serde]
pub struct AccountView {
  pub totals: AccountTotals,
  pub tickets: Vec<Ticket>,
  pub claim: Option<Claim>,
}

#[cw_serde]
pub struct ClaimView {
  pub owner: Addr,
  pub round_no: Uint64,
  pub amount: Option<Uint128>,
  pub tickets: Option<Vec<Ticket>>,
  pub matches: Vec<u16>,
  pub is_approved: bool,
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

#[cw_serde]
pub struct DryRunResponse {
  pub seed: u32,
  pub entropy: String,
  pub winning_numbers: Vec<u16>,
  pub match_counts: Vec<u16>,
}

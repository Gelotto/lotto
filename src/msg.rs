use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp, Uint128, Uint64};
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
  SetConfig {
    config: Config,
  },
  Buy {
    player: Option<Addr>,
    tickets: Vec<Vec<u16>>,
  },
  BuySeed {
    player: Option<Addr>,
    count: u16,
    seed: u32,
  },
  Draw {
    entropy: Option<String>,
  },
  Claim {},
  Withdraw {},
  Approve {
    address: Addr,
  },
  Reject {
    address: Addr,
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
  DryRun {
    ticket_count: u16,
    seed: u32,
    entropy: String,
    time: Option<Timestamp>,
    height: Option<Uint64>,
    tx_index: Option<Uint64>,
  },
  ClaimsPendingApproval {},
}

#[cw_serde]
pub enum MigrateMsg {
  V0_0_9 {},
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

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp, Uint128, Uint64};
use cw_lib::models::Token;

#[cw_serde]
pub enum RoundStatus {
  Active,
  Drawing,
}

#[cw_serde]
pub struct Config {
  pub token: Token,
  pub price: Uint128,
  pub max_number: u16,
  pub number_count: u8,
  pub round_seconds: Uint64,
  pub house_address: Addr,
  pub marketing: MarketingInfo,
  pub style: Style,
  pub payouts: Vec<Payout>,
}

#[cw_serde]
pub struct Payout {
  pub n: u8,
  pub incentive: Uint128,
  pub pct: Uint128,
}

#[cw_serde]
pub struct Round {
  pub round_no: Uint64,
  pub ticket_count: u32,
  pub start: Timestamp,
  pub end: Timestamp,
}

#[cw_serde]
pub struct MarketingInfo {
  pub name: String,
  pub description: Option<String>,
}

#[cw_serde]
pub struct Style {
  pub bg: StyleValue,
  pub colors: Vec<String>,
  pub font: Option<String>,
}

#[cw_serde]
pub enum StyleValue {
  Str(String),
  Url(String),
}

#[cw_serde]
pub struct Drawing {
  pub total_ticket_count: u32,
  pub total_balance: Uint128,
  pub processed_ticket_count: u32,
  pub cursor: Option<(u64, Addr, String)>,
  pub winning_numbers: Vec<u16>,
  pub match_counts: Vec<u16>,
}

#[cw_serde]
pub struct Win {
  pub tickets: Vec<Vec<u16>>,
  pub amount: Uint128,
  pub round_no: Uint64,
}

#[cw_serde]
pub struct Claim {
  pub round_no: Uint64,
  pub match_counts: Vec<u16>,
}

#[cw_serde]
pub struct AccountTotals {
  pub wins: u32,
  pub winnings: Uint128,
  pub tickets: u32,
}

#[cw_serde]
pub struct Account {
  pub totals: AccountTotals,
}

impl Account {
  pub fn new() -> Self {
    Self {
      totals: AccountTotals {
        wins: 0,
        winnings: Uint128::zero(),
        tickets: 0,
      },
    }
  }
}

impl Drawing {
  pub fn is_complete(&self) -> bool {
    self.total_ticket_count == self.processed_ticket_count
  }
}

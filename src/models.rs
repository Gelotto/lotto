use std::collections::HashMap;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp, Uint128, Uint64};
use cw_lib::models::Token;

use crate::{
  state::HOUSE_POT_TAX_PCT,
  util::{calc_total_claim_amount, mul_pct},
};

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
  pub rolling: bool,
  pub min_balance: Uint128,
  pub drawer: Addr,
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
  pub status: RoundStatus,
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
  pub logo: Option<String>,
}

#[cw_serde]
pub enum StyleValue {
  Str(String),
  Url(String),
}

#[cw_serde]
pub struct Drawing {
  pub round_no: Option<Uint64>,
  pub ticket_count: u32,
  pub balance: Uint128,
  pub start_balance: Uint128,
  pub pot_payout: Uint128,
  pub incentive_payout: Uint128,
  pub total_payout: Uint128,
  pub processed_ticket_count: u32,
  pub cursor: Option<(Addr, String)>,
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
  pub amount: Option<Uint128>,
  pub tickets: Vec<Vec<u16>>,
  pub matches: Vec<u16>,
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
    self.ticket_count == self.processed_ticket_count
  }

  pub fn get_total_payout(&self) -> Uint128 {
    self.pot_payout + self.incentive_payout
  }

  pub fn get_total_payout_after_tax(&self) -> Uint128 {
    (self.pot_payout - mul_pct(self.pot_payout, HOUSE_POT_TAX_PCT.into())) + self.incentive_payout
  }

  pub fn get_pot_size(&self) -> Uint128 {
    self.start_balance + self.balance
  }
}

impl Claim {
  pub fn set_amount(
    &mut self,
    drawing: &Drawing,
    payouts: &HashMap<u8, Payout>,
  ) {
    self.amount = Some(calc_total_claim_amount(self, drawing, payouts))
  }
}

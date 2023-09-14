use std::collections::{HashMap, HashSet};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Timestamp, Uint128, Uint64};
use cw_lib::models::Token;

use crate::{error::ContractError, util::calc_total_claim_amount};

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
  pub batch_size: Option<u16>,
  pub use_approval: Option<bool>,
  pub nois_proxy: Option<Addr>,
}

#[cw_serde]
pub struct Ticket {
  pub numbers: Vec<u16>,
  pub n: u16,
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
  pub balance: Uint128,
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
  pub round_balance: Uint128, // TODO: rename to end_balance
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
  pub tickets: Option<Vec<Ticket>>,
  pub matches: Vec<u16>,
  pub is_approved: bool,
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

impl Config {
  pub fn validate(
    &self,
    api: &dyn Api,
  ) -> Result<(), ContractError> {
    if let Some(proxy_addr) = &self.nois_proxy {
      api
        .addr_validate(proxy_addr.as_str())
        .map_err(|_| ContractError::ValidationError)?;
    }

    api
      .addr_validate(self.drawer.as_str())
      .map_err(|_| ContractError::ValidationError)?;

    api
      .addr_validate(self.house_address.as_str())
      .map_err(|_| ContractError::ValidationError)?;

    if let Token::Cw20 { address } = &self.token {
      api
        .addr_validate(address.as_str())
        .map_err(|_| ContractError::ValidationError)?;
    }

    if self.price.is_zero()
      || self.max_number == 0
      || self.number_count == 0
      || self.round_seconds < Uint64::from(60u64)
    {
      return Err(ContractError::ValidationError);
    }

    let mut visited: HashSet<u8> = HashSet::with_capacity(self.number_count as usize);
    for payout in self.payouts.iter() {
      if payout.pct > Uint128::from(1_000_000u128)
        || payout.n == 0
        || payout.n > self.number_count
        || visited.contains(&payout.n)
      {
        return Err(ContractError::ValidationError);
      }
      visited.insert(payout.n);
    }

    Ok(())
  }
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

  pub fn resolve_total_payout(&self) -> Uint128 {
    self.pot_payout + self.incentive_payout
  }

  pub fn resolve_pot_size(&self) -> Uint128 {
    self.start_balance + self.round_balance
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

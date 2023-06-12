use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Uint128, Uint64};
use cw_lib::models::Token;

#[cw_serde]
pub struct Config {
  pub token: Token,
  pub price: Uint128,
  pub max_number: u16,
  pub number_count: u8,
  pub round_seconds: Uint64,
  pub marketing: MarketingInfo,
  pub style: Style,
}

#[cw_serde]
pub struct MarketingInfo {
  pub name: String,
  pub description: Option<String>,
}

#[cw_serde]
pub struct Style {
  pub bg: StyleValue,
  pub colors: Vec<StyleValue>,
  pub font: Option<String>,
}

#[cw_serde]
pub enum StyleValue {
  String(String),
  Number(u32),
  Url(String),
}

#[cw_serde]
pub struct PlayerAccount {
  pub win_count: u32,
  pub total_ticket_count: u32,
  pub total_win_amount: Uint128,
}

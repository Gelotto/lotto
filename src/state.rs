use std::collections::{HashMap, HashSet};

use crate::models::{Account, Claim, Drawing, Payout, RoundStatus, Style};
use crate::msg::InstantiateMsg;
use crate::{error::ContractError, models::MarketingInfo};
use cosmwasm_std::{
  Addr, Deps, DepsMut, Env, MessageInfo, Order, Storage, Timestamp, Uint128, Uint64,
};
use cw_acl::client::Acl;
use cw_lib::models::{Owner, Token};
use cw_storage_plus::{Item, Map};

pub const CONFIG_TOKEN: Item<Token> = Item::new("config_token");
pub const CONFIG_PRICE: Item<Uint128> = Item::new("config_price");
pub const CONFIG_NUMBER_COUNT: Item<u8> = Item::new("config_number_count");
pub const CONFIG_MAX_NUMBER: Item<u16> = Item::new("config_max_number");
pub const CONFIG_ROUND_SECONDS: Item<Uint64> = Item::new("config_round_seconds");
pub const CONFIG_MARKETING: Item<MarketingInfo> = Item::new("config_marketing");
pub const CONFIG_STYLE: Item<Style> = Item::new("config_style");
pub const CONFIG_HOUSE_ADDR: Item<Addr> = Item::new("config_house_address");
pub const CONFIG_PAYOUTS: Map<u8, Payout> = Map::new("config_payouts");

pub const OWNER: Item<Owner> = Item::new("owner");
pub const ACCOUNTS: Map<Addr, Account> = Map::new("accounts");
pub const TAXES: Map<Addr, Uint128> = Map::new("taxes");

pub const ROUND_STATUS: Item<RoundStatus> = Item::new("game_state");
pub const ROUND_NO: Item<Uint64> = Item::new("round_counter");
pub const ROUND_START: Item<Timestamp> = Item::new("round_start");
pub const ROUND_TICKET_COUNT: Item<u32> = Item::new("round_ticket_count");
pub const ROUND_TICKETS: Map<(u64, Addr, String), Vec<u16>> = Map::new("round_tickets");

pub const CLAIMS: Map<Addr, Claim> = Map::new("claims");
pub const DRAWINGS: Map<u64, Drawing> = Map::new("drawings");
pub const WINNING_TICKETS: Map<(u64, Addr, String), Vec<u16>> = Map::new("winning_tickets");

pub fn initialize(
  deps: DepsMut,
  env: &Env,
  info: &MessageInfo,
  msg: &InstantiateMsg,
) -> Result<(), ContractError> {
  ROUND_NO.save(deps.storage, &Uint64::one())?;
  ROUND_START.save(deps.storage, &env.block.time)?;
  ROUND_TICKET_COUNT.save(deps.storage, &0)?;
  ROUND_STATUS.save(deps.storage, &RoundStatus::Active)?;
  OWNER.save(
    deps.storage,
    &msg
      .owner
      .clone()
      .unwrap_or_else(|| Owner::Address(info.sender.clone())),
  )?;

  for payout in msg.config.payouts.iter() {
    CONFIG_PAYOUTS.save(deps.storage, payout.n, payout)?;
  }

  CONFIG_TOKEN.save(deps.storage, &msg.config.token)?;
  CONFIG_PRICE.save(deps.storage, &msg.config.price)?;
  CONFIG_NUMBER_COUNT.save(deps.storage, &msg.config.number_count)?;
  CONFIG_MAX_NUMBER.save(deps.storage, &msg.config.max_number)?;
  CONFIG_ROUND_SECONDS.save(deps.storage, &msg.config.round_seconds)?;
  CONFIG_MARKETING.save(deps.storage, &msg.config.marketing)?;
  CONFIG_HOUSE_ADDR.save(deps.storage, &msg.config.house_address)?;
  CONFIG_STYLE.save(deps.storage, &msg.config.style)?;

  Ok(())
}

/// Helper function that returns true if given wallet (principal) is authorized
/// by ACL to the given action.
pub fn ensure_sender_is_allowed(
  deps: &Deps,
  principal: &Addr,
  action: &str,
) -> Result<(), ContractError> {
  if !match OWNER.load(deps.storage)? {
    Owner::Address(addr) => *principal == addr,
    Owner::Acl(acl_addr) => {
      let acl = Acl::new(&acl_addr);
      acl.is_allowed(&deps.querier, principal, action)?
    },
  } {
    Err(ContractError::NotAuthorized {})
  } else {
    Ok(())
  }
}

pub fn require_active_game_state(storage: &dyn Storage) -> Result<bool, ContractError> {
  Ok(ROUND_STATUS.load(storage)? == RoundStatus::Active)
}

pub fn load_account(
  storage: &dyn Storage,
  owner: &Addr,
) -> Result<Account, ContractError> {
  ACCOUNTS
    .load(storage, owner.clone())
    .map_err(|_| ContractError::AccountNotFound)
}

pub fn load_claim(
  storage: &dyn Storage,
  owner: &Addr,
) -> Result<Claim, ContractError> {
  CLAIMS
    .load(storage, owner.clone())
    .map_err(|_| ContractError::ClaimNotFound)
}

pub fn load_drawing(
  storage: &dyn Storage,
  round_no: Uint64,
) -> Result<Drawing, ContractError> {
  DRAWINGS
    .load(storage, round_no.into())
    .map_err(|_| ContractError::DrawingNotFound)
}

pub fn load_payouts(storage: &dyn Storage) -> Result<HashMap<u8, Payout>, ContractError> {
  let mut payouts: HashMap<u8, Payout> = HashMap::with_capacity(2);
  CONFIG_PAYOUTS
    .range(storage, None, None, Order::Ascending)
    .for_each(|result| {
      let (n, p) = result.unwrap();
      payouts.insert(n, p);
    });
  Ok(payouts)
}

pub fn load_winning_numbers(
  storage: &dyn Storage,
  round_no: u64,
) -> Result<HashSet<u16>, ContractError> {
  if let Some(drawing) = DRAWINGS.may_load(storage, round_no.into())? {
    Ok(HashSet::from_iter(
      drawing.winning_numbers.iter().map(|x| *x),
    ))
  } else {
    Err(ContractError::InvalidRoundNo)
  }
}

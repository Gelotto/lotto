use crate::models::{PlayerAccount, Style};
use crate::msg::InstantiateMsg;
use crate::{error::ContractError, models::MarketingInfo};
use cosmwasm_std::{Addr, DepsMut, Env, MessageInfo, Uint128, Uint64};
use cw_lib::models::{Owner, Token};
use cw_storage_plus::{Item, Map};

pub type TicketKey = (Addr, String);
pub type LutabKey = (String, Addr);

pub const CONFIG_TOKEN: Item<Token> = Item::new("token");
pub const CONFIG_PRICE: Item<Uint128> = Item::new("price");
pub const CONFIG_NUMBER_COUNT: Item<u8> = Item::new("number_count");
pub const CONFIG_MAX_NUMBER: Item<u16> = Item::new("max_number");
pub const CONFIG_ROUND_SECONDS: Item<Uint64> = Item::new("round_seconds");
pub const CONFIG_MARKETING: Item<MarketingInfo> = Item::new("marketing");
pub const CONFIG_STYLE: Item<Style> = Item::new("style");

pub const OWNER: Item<Owner> = Item::new("owner");
pub const ROUND_COUNTER: Item<Uint64> = Item::new("round_counter");
pub const TICKETS: Map<TicketKey, bool> = Map::new("tickets");
pub const LOOKUP_TABLE: Map<LutabKey, bool> = Map::new("lookup_table");
pub const TICKET_COUNT: Item<u32> = Item::new("ticket_count");
pub const TAX_RATES: Map<Addr, Uint128> = Map::new("tax_rates");
pub const ACCOUNTS: Map<Addr, PlayerAccount> = Map::new("accounts");

pub fn initialize(
  deps: DepsMut,
  _env: &Env,
  info: &MessageInfo,
  msg: &InstantiateMsg,
) -> Result<(), ContractError> {
  ROUND_COUNTER.save(deps.storage, &Uint64::one())?;
  OWNER.save(
    deps.storage,
    &msg
      .owner
      .clone()
      .unwrap_or_else(|| Owner::Address(info.sender.clone())),
  )?;
  Ok(())
}

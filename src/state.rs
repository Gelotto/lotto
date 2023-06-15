use crate::models::{PlayerAccount, Style};
use crate::msg::InstantiateMsg;
use crate::{error::ContractError, models::MarketingInfo};
use cosmwasm_std::{Addr, DepsMut, Env, MessageInfo, Timestamp, Uint128, Uint64};
use cw_lib::models::{Owner, Token};
use cw_storage_plus::{Item, Map};

pub type TicketKey = (Addr, String);
pub type LutabKey = (String, Addr);

pub const MAX_RECENT_WINS_LEN: usize = 10;

pub const CONFIG_TOKEN: Item<Token> = Item::new("token");
pub const CONFIG_PRICE: Item<Uint128> = Item::new("price");
pub const CONFIG_NUMBER_COUNT: Item<u8> = Item::new("number_count");
pub const CONFIG_MAX_NUMBER: Item<u16> = Item::new("max_number");
pub const CONFIG_MAX_TICKETS_PER_ROUND: Item<u16> = Item::new("max_tickets_per_round");
pub const CONFIG_ROUND_SECONDS: Item<Uint64> = Item::new("round_seconds");
pub const CONFIG_MARKETING: Item<MarketingInfo> = Item::new("marketing");
pub const CONFIG_STYLE: Item<Style> = Item::new("style");

pub const OWNER: Item<Owner> = Item::new("owner");
pub const ROUND_COUNTER: Item<Uint64> = Item::new("round_counter");
pub const ROUND_START: Item<Timestamp> = Item::new("round_start");
pub const TICKET_COUNT: Item<u32> = Item::new("ticket_count");
pub const TICKETS: Map<TicketKey, bool> = Map::new("tickets");
pub const LOOKUP_TABLE: Map<LutabKey, bool> = Map::new("lookup_table");
pub const TAX_RATES: Map<Addr, Uint128> = Map::new("tax_rates");
pub const ACCOUNTS: Map<Addr, PlayerAccount> = Map::new("accounts");

pub fn initialize(
  deps: DepsMut,
  env: &Env,
  info: &MessageInfo,
  msg: &InstantiateMsg,
) -> Result<(), ContractError> {
  ROUND_COUNTER.save(deps.storage, &Uint64::one())?;
  ROUND_START.save(deps.storage, &env.block.time)?;
  OWNER.save(
    deps.storage,
    &msg
      .owner
      .clone()
      .unwrap_or_else(|| Owner::Address(info.sender.clone())),
  )?;

  CONFIG_TOKEN.save(deps.storage, &msg.config.token)?;
  CONFIG_PRICE.save(deps.storage, &msg.config.price)?;
  CONFIG_NUMBER_COUNT.save(deps.storage, &msg.config.number_count)?;
  CONFIG_MAX_NUMBER.save(deps.storage, &msg.config.max_number)?;
  CONFIG_MAX_TICKETS_PER_ROUND.save(deps.storage, &msg.config.max_tickets_per_round)?;
  CONFIG_ROUND_SECONDS.save(deps.storage, &msg.config.round_seconds)?;
  CONFIG_MARKETING.save(deps.storage, &msg.config.marketing)?;
  CONFIG_STYLE.save(deps.storage, &msg.config.style)?;

  Ok(())
}

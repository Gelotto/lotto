use std::collections::{HashMap, HashSet};

use crate::models::{Account, Claim, Config, Drawing, Payout, RoundStatus, Style, Ticket};
use crate::msg::InstantiateMsg;
use crate::util::calc_total_claim_amount;
use crate::xorshift32::Xorshift32;
use crate::{error::ContractError, models::MarketingInfo};
use cosmwasm_std::{
  Addr, BlockInfo, Deps, DepsMut, Env, MessageInfo, Order, Storage, SubMsg, Timestamp, Uint128,
  Uint64,
};
use cw_acl::client::Acl;
use cw_lib::models::{Owner, Token};
use cw_lib::random::{Pcg64, RngComponent};
use cw_lib::utils::funds::build_send_submsg;
use cw_storage_plus::{Item, Map};
use house_staking::client::House;

pub const HOUSE_TICKET_TAX_PCT: u128 = 5_0000; // 5%
pub const HOUSE_POT_TAX_PCT: u128 = 10_0000; // 10%

pub const CONFIG_TOKEN: Item<Token> = Item::new("config_token");
pub const CONFIG_PRICE: Item<Uint128> = Item::new("config_price");
pub const CONFIG_NUMBER_COUNT: Item<u8> = Item::new("config_number_count");
pub const CONFIG_MAX_NUMBER: Item<u16> = Item::new("config_max_number");
pub const CONFIG_ROUND_SECONDS: Item<Uint64> = Item::new("config_round_seconds");
pub const CONFIG_MARKETING: Item<MarketingInfo> = Item::new("config_marketing");
pub const CONFIG_STYLE: Item<Style> = Item::new("config_style");
pub const CONFIG_HOUSE_ADDR: Item<Addr> = Item::new("config_house_address");
pub const CONFIG_ROLLING: Item<bool> = Item::new("config_rolling");
pub const CONFIG_MIN_BALANCE: Item<Uint128> = Item::new("config_min_balance");
pub const CONFIG_PAYOUTS: Map<u8, Payout> = Map::new("config_payouts");
pub const CONFIG_DRAWER: Item<Addr> = Item::new("config_drawer");
pub const CONFIG_TICKET_BATCH_SIZE: Item<u16> = Item::new("config_ticket_batch_size");
pub const CONFIG_USE_APPROVAL: Item<bool> = Item::new("config_use_approval");

pub const OWNER: Item<Owner> = Item::new("owner");
pub const ACCOUNTS: Map<Addr, Account> = Map::new("accounts");
pub const TAXES: Map<Addr, Uint128> = Map::new("taxes");
pub const DEBUG_WINNING_NUMBERS: Item<Option<Vec<u16>>> = Item::new("debug_winning_numbers");
pub const PREV_HEIGHT: Item<Uint64> = Item::new("prev_height");

pub const ROUND_STATUS: Item<RoundStatus> = Item::new("game_state");
pub const ROUND_NO: Item<Uint64> = Item::new("round_counter");
pub const ROUND_START: Item<Timestamp> = Item::new("round_start");
pub const ROUND_TICKET_COUNT: Item<u32> = Item::new("round_ticket_count");
pub const ROUND_TICKETS: Map<(Addr, String), Ticket> = Map::new("round_tickets");

pub const CLAIMS: Map<Addr, Claim> = Map::new("claims");
pub const JACKPOT_CLAIMANTS: Map<&Addr, bool> = Map::new("jackpot_claimants");
pub const BALANCE_CLAIMABLE: Item<Uint128> = Item::new("total_claim_amount");
pub const DRAWINGS: Map<u64, Drawing> = Map::new("drawings");
pub const STAGED_CONFIG: Item<Option<Config>> = Item::new("staged_config");

pub fn initialize(
  deps: DepsMut,
  env: &Env,
  info: &MessageInfo,
  msg: &InstantiateMsg,
) -> Result<(), ContractError> {
  let owner = msg
    .owner
    .clone()
    .unwrap_or_else(|| Owner::Address(info.sender.clone()));

  deps.api.addr_validate(msg.config.drawer.as_str())?;
  deps.api.addr_validate(msg.config.house_address.as_str())?;

  if let Token::Cw20 { address } = msg.config.token.clone() {
    deps.api.addr_validate(address.as_str())?;
  }

  if let Owner::Acl(address) = &owner {
    deps.api.addr_validate(address.as_str())?;
  } else if let Owner::Address(address) = &owner {
    deps.api.addr_validate(address.as_str())?;
  }

  ROUND_NO.save(deps.storage, &Uint64::one())?;
  ROUND_START.save(deps.storage, &env.block.time)?;
  ROUND_TICKET_COUNT.save(deps.storage, &0)?;
  ROUND_STATUS.save(deps.storage, &RoundStatus::Active)?;
  OWNER.save(deps.storage, &owner)?;
  BALANCE_CLAIMABLE.save(deps.storage, &Uint128::zero())?;
  STAGED_CONFIG.save(deps.storage, &None)?;
  PREV_HEIGHT.save(deps.storage, &env.block.height.into())?;

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
  CONFIG_ROLLING.save(deps.storage, &msg.config.rolling)?;
  CONFIG_MIN_BALANCE.save(deps.storage, &msg.config.min_balance)?;
  CONFIG_STYLE.save(deps.storage, &msg.config.style)?;
  CONFIG_DRAWER.save(deps.storage, &msg.config.drawer)?;
  CONFIG_USE_APPROVAL.save(deps.storage, &msg.config.use_approval.unwrap_or(false))?;
  CONFIG_TICKET_BATCH_SIZE.save(
    deps.storage,
    &msg.config.batch_size.unwrap_or(1000).clamp(100, 1000),
  )?;

  DEBUG_WINNING_NUMBERS.save(deps.storage, &msg.winning_numbers)?;

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

pub fn require_active_game_state(storage: &dyn Storage) -> Result<(), ContractError> {
  if ROUND_STATUS.load(storage)? != RoundStatus::Active {
    Err(ContractError::NotActive)
  } else {
    Ok(())
  }
}

pub fn load_house(storage: &dyn Storage) -> Result<House, ContractError> {
  Ok(House::new(&CONFIG_HOUSE_ADDR.load(storage)?))
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

pub fn is_ready(
  storage: &dyn Storage,
  block: &BlockInfo,
) -> Result<bool, ContractError> {
  let prev_height = PREV_HEIGHT.load(storage)?.u64();
  if block.height <= prev_height {
    return Ok(false);
  }
  let status = ROUND_STATUS.load(storage)?;
  if RoundStatus::Active == status {
    let round_start = ROUND_START.load(storage)?;
    let round_duration = CONFIG_ROUND_SECONDS.load(storage)?;
    // Abort if the round hasn't reach its end time
    if (round_start.seconds() + round_duration.u64()) > block.time.seconds() {
      return Ok(false);
    }
  }
  Ok(true)
}

pub fn load_latest_drawing(storage: &dyn Storage) -> Result<Option<Drawing>, ContractError> {
  let mut round_no = ROUND_NO.load(storage).unwrap();
  if round_no > Uint64::one() {
    round_no -= Uint64::one();
  }
  Ok(DRAWINGS.may_load(storage, round_no.u64())?)
}

pub fn load_claim_tickets_by_account(
  storage: &dyn Storage,
  address: &Addr,
) -> Result<Vec<Ticket>, ContractError> {
  let map_tag = format!("claim_tickets_{}", address.to_string());
  let map: Map<String, Ticket> = Map::new(map_tag.as_str());
  Ok(
    map
      .range(storage, None, None, Order::Ascending)
      .map(|r| r.unwrap().1)
      .collect(),
  )
}

pub fn process_claim(
  storage: &mut dyn Storage,
  sender: &Addr,
  claim: Claim,
  is_rejected: bool,
) -> Result<Option<SubMsg>, ContractError> {
  let drawing = load_drawing(storage, claim.round_no)?;
  let payouts = load_payouts(storage)?;
  let token = CONFIG_TOKEN.load(storage)?;
  let claim_amount = calc_total_claim_amount(&claim, &drawing, &payouts);

  // If this claim is for a jackpot, clear it from state that tracks this fact.
  if JACKPOT_CLAIMANTS.has(storage, sender) {
    JACKPOT_CLAIMANTS.remove(storage, sender);
  }

  // Remove the claim
  CLAIMS.remove(storage, sender.clone());

  // clear the Claim's tickets from state
  let map_tag = format!("claim_tickets_{}", sender.to_string());
  let map: Map<String, Ticket> = Map::new(map_tag.as_str());
  map.clear(storage);

  BALANCE_CLAIMABLE.update(storage, |total| -> Result<_, ContractError> {
    Ok(total - claim_amount)
  })?;

  if is_rejected {
    Ok(None)
  } else {
    ACCOUNTS.update(
      storage,
      sender.clone(),
      |maybe_account| -> Result<_, ContractError> {
        if let Some(mut account) = maybe_account {
          account.totals.winnings = claim_amount;
          account.totals.wins += claim.matches.iter().map(|x| *x as u32).sum::<u32>();
          Ok(account)
        } else {
          Err(ContractError::AccountNotFound)
        }
      },
    )?;

    Ok(if claim_amount.is_zero() {
      None
    } else {
      Some(build_send_submsg(&sender, claim_amount, &token)?)
    })
  }
}

pub fn draw_winning_numbers(
  storage: &dyn Storage,
  env: &Env,
  entropy: &String,
  maybe_time: Option<Timestamp>,
  maybe_height: Option<u64>,
  maybe_tx_index: Option<u64>,
) -> Result<HashSet<u16>, ContractError> {
  let round_no = ROUND_NO.load(storage)?;
  let time = maybe_time.unwrap_or(env.block.time);
  let height = maybe_height.unwrap_or(env.block.height);
  let tx_index = maybe_tx_index.unwrap_or_else(|| {
    env
      .transaction
      .clone()
      .and_then(|t| Some(t.index as u64))
      .unwrap_or(0u64)
  });

  let numbers = generate_random_numbers(
    storage,
    time,
    height,
    tx_index,
    &env.contract.address,
    round_no,
    entropy,
  )?;
  Ok(HashSet::from_iter(numbers.iter().map(|x| *x)))
}

pub fn generate_random_numbers(
  storage: &dyn Storage,
  time: Timestamp,
  height: u64,
  tx_index: u64,
  contract_addr: &Addr,
  round_no: Uint64,
  entropy: &String,
) -> Result<Vec<u16>, ContractError> {
  if let Some(debug_winning_numbers) = DEBUG_WINNING_NUMBERS.load(storage)? {
    return Ok(debug_winning_numbers);
  }

  let number_count = CONFIG_NUMBER_COUNT.load(storage)?;
  let max_value = CONFIG_MAX_NUMBER.load(storage)?;
  let mut winning_numbers: HashSet<u16> = HashSet::with_capacity(number_count as usize);
  let mut rng = Pcg64::from_components(&vec![
    RngComponent::Int(round_no.u64()),
    RngComponent::Str(entropy.clone()),
    RngComponent::Str(contract_addr.to_string()),
    RngComponent::Int(height),
    RngComponent::Int(time.nanos()),
    RngComponent::Int(tx_index),
  ]);

  while winning_numbers.len() < number_count as usize {
    winning_numbers.insert((rng.next_u64() % ((max_value + 1) as u64)) as u16);
  }

  Ok(winning_numbers.iter().map(|x| *x).collect())
}

pub fn generate_random_tickets(
  storage: &dyn Storage,
  ticket_count: u16,
  seed: u32,
) -> Result<Vec<Vec<u16>>, ContractError> {
  let number_count = CONFIG_NUMBER_COUNT.load(storage)?;
  let max_val = CONFIG_MAX_NUMBER.load(storage)?;
  let mut tickets: Vec<Vec<u16>> = Vec::with_capacity(ticket_count as usize);
  let mut visited: HashSet<u16> = HashSet::with_capacity(number_count as usize - 1);
  let mut rng = Xorshift32::new(seed);
  for _ in 0..ticket_count {
    let mut numbers: Vec<u16> = Vec::with_capacity(number_count as usize);
    while numbers.len() < number_count as usize {
      let x = rng
        .random_int_in_range(0, max_val.into())
        .clamp(0, u16::MAX as u32) as u16;
      if !visited.contains(&x) {
        numbers.push(x);
        visited.insert(x);
      }
    }
    tickets.push(numbers);
    visited.clear();
  }
  Ok(tickets)
}

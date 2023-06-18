use std::collections::HashSet;

use crate::{
  error::ContractError,
  models::Account,
  state::{
    ACCOUNTS, CONFIG_MAX_NUMBER, CONFIG_NUMBER_COUNT, CONFIG_PRICE, CONFIG_TOKEN, ROUND_NO,
    ROUND_TICKETS, ROUND_TICKET_COUNT,
  },
  util::hash_numbers,
};
use cosmwasm_std::{
  attr, Addr, Coin, DepsMut, Empty, Env, MessageInfo, QuerierWrapper, Response, Storage, Uint128,
  Uint64, WasmMsg,
};
use cw_lib::{
  models::Token,
  utils::funds::{build_cw20_transfer_from_msg, get_cw20_balance, has_funds},
};

pub fn buy(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  tickets: Vec<Vec<u16>>,
) -> Result<Response, ContractError> {
  // Abort if sender has tickets in ticket map from the last round they played.

  let ticket_price = CONFIG_PRICE.load(deps.storage)?;
  let round_no = ROUND_NO.load(deps.storage)?;
  let total_price = Uint128::from(tickets.len() as u64) * ticket_price;

  // Upsert player account
  ACCOUNTS.update(
    deps.storage,
    info.sender.clone(),
    |maybe_account| -> Result<_, ContractError> {
      if let Some(mut account) = maybe_account {
        account.totals.tickets += tickets.len() as u32;
        Ok(account)
      } else {
        let mut account = Account::new();
        account.totals.tickets = tickets.len() as u32;
        Ok(account)
      }
    },
  )?;

  // Process each ticket ordered, updating state
  for numbers in tickets.iter() {
    process_ticket(deps.storage, &info, round_no, numbers.clone())?;
  }

  // Ensure required balance & transfer_from msg if appropriate
  let resp = Response::new().add_attributes(vec![attr("action", "buy")]);

  Ok(
    if let Some(msg) = take_payment(
      deps.storage,
      deps.querier,
      &env.contract.address,
      &info.funds,
      &info.sender,
      total_price,
    )? {
      resp.add_message(msg)
    } else {
      resp
    },
  )
}

pub fn process_ticket(
  storage: &mut dyn Storage,
  info: &MessageInfo,
  round_no: Uint64,
  numbers: Vec<u16>,
) -> Result<(), ContractError> {
  require_valid_numbers(storage, numbers.clone())?;

  // TODO: auto-claim record exists

  // sort the numbers
  let mut sorted_numbers = numbers.clone();
  sorted_numbers.sort();

  // Build key into ticket map
  let hash = hash_numbers(&sorted_numbers);
  let key = (round_no.into(), info.sender.clone(), hash);

  // Insert the ticket or error out if the sender already has one.
  ROUND_TICKETS.update(storage, key, |x| -> Result<_, ContractError> {
    if x.is_some() {
      Err(ContractError::TicketExists)
    } else {
      Ok(sorted_numbers)
    }
  })?;

  // Increase the round's current ticket count
  ROUND_TICKET_COUNT.update(storage, |n| -> Result<_, ContractError> { Ok(n + 1) })?;

  Ok(())
}

fn require_valid_numbers(
  storage: &dyn Storage,
  numbers: Vec<u16>,
) -> Result<(), ContractError> {
  // Ensure we have the right amount of numbers
  let required_number_count = CONFIG_NUMBER_COUNT.load(storage)?;
  if numbers.len() != required_number_count as usize {
    return Err(ContractError::InvalidNumberCount);
  }

  // Ensure each number is within the allowed range
  let max_value = CONFIG_MAX_NUMBER.load(storage)?;
  let mut visited: HashSet<u16> = HashSet::with_capacity(numbers.len());
  for n in numbers.iter() {
    if visited.contains(n) {
      return Err(ContractError::DuplicateNumber);
    }
    if *n > max_value {
      return Err(ContractError::NumberOutOfBounds);
    }
    visited.insert(*n);
  }

  Ok(())
}

fn take_payment(
  storage: &dyn Storage,
  querier: QuerierWrapper<Empty>,
  contract_address: &Addr,
  funds: &Vec<Coin>,
  sender: &Addr,
  amount: Uint128,
) -> Result<Option<WasmMsg>, ContractError> {
  Ok(match CONFIG_TOKEN.load(storage)? {
    // Take native token payment (namely, Juno or some other ibc denom)
    Token::Native { denom } => {
      if !has_funds(funds, amount, &denom) {
        return Err(ContractError::InsufficientFunds);
      }
      None
    },
    // Take CW20 payment
    Token::Cw20 {
      address: cw20_address,
    } => {
      let balance = get_cw20_balance(querier, &cw20_address, sender)?;
      if balance < amount {
        return Err(ContractError::InsufficientFunds);
      }
      Some(build_cw20_transfer_from_msg(
        sender,
        contract_address,
        &cw20_address,
        amount,
      )?)
    },
  })
}

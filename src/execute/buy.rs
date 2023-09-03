use std::collections::HashSet;

use crate::{
  error::ContractError,
  models::{Account, Ticket},
  state::{
    generate_random_tickets, load_house, require_active_game_state, ACCOUNTS, CONFIG_MAX_NUMBER,
    CONFIG_NUMBER_COUNT, CONFIG_PRICE, CONFIG_TOKEN, HOUSE_TICKET_TAX_PCT, ROUND_TICKETS,
    ROUND_TICKET_COUNT,
  },
  util::{hash_numbers, mul_pct},
};
use cosmwasm_std::{
  attr, Addr, Coin, DepsMut, Empty, Env, MessageInfo, QuerierWrapper, Response, Storage, Uint128,
  WasmMsg,
};
use cw_lib::{
  models::Token,
  utils::funds::{build_cw20_transfer_from_msg, get_cw20_balance, has_funds},
};
use house_staking::models::AccountTokenAmount;

pub fn buy_seed(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  maybe_player: Option<Addr>,
  ticket_count: u16,
  seed: u32,
) -> Result<Response, ContractError> {
  let tickets = generate_random_tickets(deps.storage, ticket_count, seed)?;
  buy(deps, env, info, maybe_player, tickets)
}

pub fn buy(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  maybe_player: Option<Addr>,
  tickets: Vec<Vec<u16>>,
) -> Result<Response, ContractError> {
  // Reject attempt to buy tickets if the lotto is currently drawing.
  require_active_game_state(deps.storage)?;

  // The player is the address on whose behalf tickets are bought. If not
  // explicitly defined, default to the tx sender.
  let player = maybe_player.unwrap_or(info.sender.clone());

  // Upsert player account
  ACCOUNTS.update(
    deps.storage,
    player.clone(),
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
    process_ticket(deps.storage, &player, numbers.clone())?;
  }

  let ticket_price = CONFIG_PRICE.load(deps.storage)?;
  let total_price = Uint128::from(tickets.len() as u64) * ticket_price;

  let mut resp = Response::new().add_attributes(vec![attr("action", "buy")]);

  // Ensure funds and take payment from sender
  if let Some(msg) = take_payment(
    deps.storage,
    deps.querier,
    &env.contract.address,
    &info.funds,
    &info.sender,
    total_price,
  )? {
    resp = resp.add_message(msg);
  };

  // Send the house its revenue (5% of ticket proceeds)
  let token = CONFIG_TOKEN.load(deps.storage)?;
  let house_take = mul_pct(total_price, HOUSE_TICKET_TAX_PCT.into());
  let house = load_house(deps.storage)?;

  resp = resp.add_messages(house.process(
    info.sender.clone(),
    Some(AccountTokenAmount::new(&env.contract.address, house_take)),
    None,
    Some(info.funds),
    if let Token::Cw20 { address } = token {
      Some(address)
    } else {
      None
    },
  )?);

  Ok(resp)
}

fn process_ticket(
  storage: &mut dyn Storage,
  player: &Addr,
  numbers: Vec<u16>,
) -> Result<(), ContractError> {
  require_valid_numbers(storage, numbers.clone())?;

  // sort the numbers
  let mut sorted_numbers = numbers.clone();
  sorted_numbers.sort();

  // Build key into ticket map
  let hash = hash_numbers(&sorted_numbers);
  let key = (player.clone(), hash);

  // While the ticket number hash is sorted, the vec stored in the map's values
  // is not. This can hypothetically let us check whether the ticket matches
  // with respect to order (permutations rather than combinations).
  ROUND_TICKETS.update(storage, key, |maybe_ticket| -> Result<_, ContractError> {
    if let Some(mut ticket) = maybe_ticket {
      ticket.n += 1;
      Ok(ticket)
    } else {
      Ok(Ticket { numbers, n: 1 })
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

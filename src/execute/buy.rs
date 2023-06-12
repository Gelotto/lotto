use crate::{
  error::ContractError,
  models::PlayerAccount,
  state::{
    LutabKey, TicketKey, ACCOUNTS, CONFIG_MAX_NUMBER, CONFIG_PRICE, CONFIG_TOKEN, LOOKUP_TABLE,
    TICKETS, TICKET_COUNT,
  },
  util::hash_numbers,
};
use cosmwasm_std::{
  attr, Addr, Coin, DepsMut, Empty, Env, MessageInfo, QuerierWrapper, Response, Storage, Uint128,
  WasmMsg,
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
  let ticket_price = CONFIG_PRICE.load(deps.storage)?;
  let total_price = Uint128::from(tickets.len() as u64) * ticket_price;

  // Process each ticket ordered, updating state
  for numbers in tickets.iter() {
    process_ticket(deps.storage, &info, numbers.clone())?;
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
  numbers: Vec<u16>,
) -> Result<(), ContractError> {
  require_valid_numbers(storage, &numbers)?;

  // Build key into ticket map.
  let (ticket_key, lutab_key) = build_keys(&info.sender, &numbers)?;

  // Insert the ticket or error out if the sender already has one.
  TICKETS.update(storage, ticket_key, |x| -> Result<_, ContractError> {
    if x.is_some() {
      Err(ContractError::TicketExists)
    } else {
      Ok(true)
    }
  })?;

  // Save entry in lookup table used to draw the winners.
  LOOKUP_TABLE.save(storage, lutab_key, &true)?;

  // Increase the round's current ticket count
  TICKET_COUNT.update(storage, |n| -> Result<_, ContractError> { Ok(n + 1) })?;

  // Upsert player account
  ACCOUNTS.update(
    storage,
    info.sender.clone(),
    |maybe_account| -> Result<_, ContractError> {
      if let Some(mut account) = maybe_account {
        account.total_ticket_count += 1;
        Ok(account)
      } else {
        Ok(PlayerAccount {
          total_ticket_count: 1,
          win_count: 0,
          total_win_amount: Uint128::zero(),
          recent_wins: vec![],
        })
      }
    },
  )?;

  Ok(())
}

fn build_keys(
  sender: &Addr,
  numbers: &Vec<u16>,
) -> Result<(TicketKey, LutabKey), ContractError> {
  let hash = hash_numbers(&numbers);
  let ticket_key = (sender.clone(), hash.clone());
  let lutab_key = (hash, sender.clone());
  Ok((ticket_key, lutab_key))
}

fn require_valid_numbers(
  storage: &dyn Storage,
  numbers: &Vec<u16>,
) -> Result<(), ContractError> {
  // Ensure we have the right amount of numbers
  let required_number_count = CONFIG_MAX_NUMBER.load(storage)?;
  if numbers.len() != required_number_count as usize {
    return Err(ContractError::InvalidNumberCount);
  }

  // Ensure each number is within the allowed range
  let max_value = CONFIG_MAX_NUMBER.load(storage)?;
  for n in numbers.iter() {
    if *n > max_value {
      return Err(ContractError::NumberOutOfBounds);
    }
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

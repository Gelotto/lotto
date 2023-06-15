use crate::error::ContractError;
use crate::models::Config;
use crate::state::{
  ACCOUNTS, CONFIG_MARKETING, CONFIG_MAX_NUMBER, CONFIG_MAX_TICKETS_PER_ROUND, CONFIG_NUMBER_COUNT,
  CONFIG_PRICE, CONFIG_ROUND_SECONDS, CONFIG_STYLE, CONFIG_TOKEN, ROUND_COUNTER, ROUND_START,
  TAX_RATES, TICKET_COUNT,
};
use crate::{msg::SelectResponse, state::OWNER};
use cosmwasm_std::{Addr, Deps, Env, Order};
use cw_lib::utils::funds::get_token_balance;
use cw_repository::client::Repository;

pub fn select(
  deps: Deps,
  env: Env,
  maybe_fields: Option<Vec<String>>,
  maybe_account: Option<Addr>,
) -> Result<SelectResponse, ContractError> {
  let loader = Repository::loader(deps.storage, &maybe_fields, &maybe_account);

  let round_seconds = CONFIG_ROUND_SECONDS.load(deps.storage)?;
  let round_start = ROUND_START.load(deps.storage)?;
  let token = CONFIG_TOKEN.load(deps.storage)?;

  Ok(SelectResponse {
    owner: loader.get("owner", &OWNER)?,
    round_count: loader.get("round_count", &ROUND_COUNTER)?,
    ticket_count: loader.get("ticket_count", &TICKET_COUNT)?,
    round_start: Some(round_start),
    round_end: loader.view("round_end", |_| {
      Ok(Some(round_start.plus_seconds(round_seconds.into())))
    })?,
    balance: loader.view("balance", |_| {
      Ok(Some(get_token_balance(
        deps.querier,
        &env.contract.address,
        &token,
      )?))
    })?,
    config: loader.view("config", |_| {
      Ok(Some(Config {
        marketing: CONFIG_MARKETING.load(deps.storage)?,
        max_number: CONFIG_MAX_NUMBER.load(deps.storage)?,
        max_tickets_per_round: CONFIG_MAX_TICKETS_PER_ROUND.load(deps.storage)?,
        number_count: CONFIG_NUMBER_COUNT.load(deps.storage)?,
        price: CONFIG_PRICE.load(deps.storage)?,
        style: CONFIG_STYLE.load(deps.storage)?,
        token: token.clone(),
        round_seconds,
      }))
    })?,
    tax_rate: loader.view("tax_rate", |_| {
      Ok(Some(
        TAX_RATES
          .range(deps.storage, None, None, Order::Ascending)
          .map(|r| r.unwrap().1)
          .sum(),
      ))
    })?,
    account: loader.view("account", |maybe_addr| {
      if let Some(addr) = maybe_addr {
        return Ok(ACCOUNTS.may_load(deps.storage, addr.clone())?);
      }
      Ok(None)
    })?,
  })
}

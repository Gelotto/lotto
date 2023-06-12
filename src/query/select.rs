use crate::error::ContractError;
use crate::models::Config;
use crate::state::{
  ACCOUNTS, CONFIG_MARKETING, CONFIG_MAX_NUMBER, CONFIG_MAX_TICKET_PER_ROUND, CONFIG_NUMBER_COUNT,
  CONFIG_PRICE, CONFIG_ROUND_SECONDS, CONFIG_STYLE, CONFIG_TOKEN, ROUND_COUNTER, TAX_RATES,
};
use crate::{msg::SelectResponse, state::OWNER};
use cosmwasm_std::{Addr, Deps, Order};
use cw_repository::client::Repository;

pub fn select(
  deps: Deps,
  maybe_fields: Option<Vec<String>>,
  maybe_account: Option<Addr>,
) -> Result<SelectResponse, ContractError> {
  let loader = Repository::loader(deps.storage, &maybe_fields, &maybe_account);
  Ok(SelectResponse {
    round_count: loader.get("round_count", &ROUND_COUNTER)?,
    owner: loader.get("owner", &OWNER)?,
    config: loader.view("config", |_| {
      Ok(Some(Config {
        marketing: CONFIG_MARKETING.load(deps.storage)?,
        token: CONFIG_TOKEN.load(deps.storage)?,
        max_number: CONFIG_MAX_NUMBER.load(deps.storage)?,
        max_ticket_per_round: CONFIG_MAX_TICKET_PER_ROUND.load(deps.storage)?,
        number_count: CONFIG_NUMBER_COUNT.load(deps.storage)?,
        price: CONFIG_PRICE.load(deps.storage)?,
        round_seconds: CONFIG_ROUND_SECONDS.load(deps.storage)?,
        style: CONFIG_STYLE.load(deps.storage)?,
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

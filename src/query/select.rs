use crate::error::ContractError;
use crate::models::{Config, Round};
use crate::state::{
  ACCOUNTS, CONFIG_MARKETING, CONFIG_MAX_NUMBER, CONFIG_NUMBER_COUNT, CONFIG_PAYOUTS, CONFIG_PRICE,
  CONFIG_ROUND_SECONDS, CONFIG_STYLE, CONFIG_TOKEN, ROUND_NO, ROUND_START, ROUND_TICKETS,
  ROUND_TICKET_COUNT, TAXES,
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

  let round_no = ROUND_NO.load(deps.storage)?;
  let round_seconds = CONFIG_ROUND_SECONDS.load(deps.storage)?;
  let round_start = ROUND_START.load(deps.storage)?;
  let token = CONFIG_TOKEN.load(deps.storage)?;

  Ok(SelectResponse {
    owner: loader.get("owner", &OWNER)?,

    round: loader.view("round", |_| {
      Ok(Some(Round {
        start: round_start.clone(),
        end: round_start.plus_seconds(round_seconds.into()),
        ticket_count: ROUND_TICKET_COUNT.load(deps.storage)?,
        round_no,
      }))
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
        number_count: CONFIG_NUMBER_COUNT.load(deps.storage)?,
        price: CONFIG_PRICE.load(deps.storage)?,
        style: CONFIG_STYLE.load(deps.storage)?,
        token: token.clone(),
        round_seconds,
        payouts: CONFIG_PAYOUTS
          .range(deps.storage, None, None, Order::Ascending)
          .map(|r| r.unwrap().1)
          .collect(),
      }))
    })?,

    tax_rate: loader.view("tax_rate", |_| {
      Ok(Some(
        TAXES
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

    tickets: loader.view("tickets", |maybe_addr| {
      if let Some(addr) = maybe_addr {
        return Ok(Some(
          ROUND_TICKETS
            .prefix((round_no.into(), addr))
            .range(deps.storage, None, None, Order::Ascending)
            .map(|r| r.unwrap().1)
            .collect(),
        ));
      }
      Ok(None)
    })?,
  })
}

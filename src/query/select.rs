use crate::error::ContractError;
use crate::models::{Config, Round};
use crate::msg::AccountView;
use crate::state::{
  load_claim_tickets_by_account, load_payouts, ACCOUNTS, BALANCE_CLAIMABLE, CLAIMS, CONFIG_DRAWER,
  CONFIG_HOUSE_ADDR, CONFIG_MARKETING, CONFIG_MAX_NUMBER, CONFIG_MIN_BALANCE, CONFIG_NOIS_PROXY,
  CONFIG_NUMBER_COUNT, CONFIG_PAYOUTS, CONFIG_PRICE, CONFIG_ROLLING, CONFIG_ROUND_SECONDS,
  CONFIG_STYLE, CONFIG_TICKET_BATCH_SIZE, CONFIG_TOKEN, CONFIG_USE_APPROVAL, DRAWINGS, ROUND_NO,
  ROUND_START, ROUND_STATUS, ROUND_TICKETS, ROUND_TICKET_COUNT, TAXES,
};
use crate::util::calc_total_claim_amount;
use crate::{msg::SelectResponse, state::OWNER};
use cosmwasm_std::{Addr, Deps, Env, Order};
use cw_lib::loader::StateLoader;
use cw_lib::utils::funds::get_token_balance;

pub fn select(
  deps: Deps,
  env: Env,
  maybe_fields: Option<Vec<String>>,
  maybe_account: Option<Addr>,
) -> Result<SelectResponse, ContractError> {
  let loader = StateLoader::new(deps.storage, &maybe_fields, &maybe_account);

  let round_no = ROUND_NO.load(deps.storage)?;
  let round_seconds = CONFIG_ROUND_SECONDS.load(deps.storage)?;
  let round_start = ROUND_START.load(deps.storage)?;
  let token = CONFIG_TOKEN.load(deps.storage)?;
  let min_balance = CONFIG_MIN_BALANCE.load(deps.storage)?;
  let contract_balance = get_token_balance(deps.querier, &env.contract.address, &token)?;
  let balance_claimable = BALANCE_CLAIMABLE.load(deps.storage)?;

  Ok(SelectResponse {
    owner: loader.get("owner", &OWNER)?,

    balance: loader.view("balance", || Ok(Some(contract_balance)))?,

    balance_claimable: Some(balance_claimable),

    round: loader.view("round", || {
      Ok(Some(Round {
        start: round_start.clone(),
        end: round_start.plus_seconds(round_seconds.into()),
        ticket_count: ROUND_TICKET_COUNT.load(deps.storage)?,
        status: ROUND_STATUS.load(deps.storage)?,
        balance: contract_balance - balance_claimable,
        round_no,
      }))
    })?,

    config: loader.view("config", || {
      Ok(Some(Config {
        marketing: CONFIG_MARKETING.load(deps.storage)?,
        max_number: CONFIG_MAX_NUMBER.load(deps.storage)?,
        number_count: CONFIG_NUMBER_COUNT.load(deps.storage)?,
        price: CONFIG_PRICE.load(deps.storage)?,
        style: CONFIG_STYLE.load(deps.storage)?,
        house_address: CONFIG_HOUSE_ADDR.load(deps.storage)?,
        rolling: CONFIG_ROLLING.load(deps.storage)?,
        drawer: CONFIG_DRAWER.load(deps.storage)?,
        batch_size: Some(CONFIG_TICKET_BATCH_SIZE.load(deps.storage)?),
        use_approval: Some(CONFIG_USE_APPROVAL.load(deps.storage)?),
        nois_proxy: CONFIG_NOIS_PROXY.load(deps.storage).unwrap_or(None),
        token: token.clone(),
        round_seconds,
        min_balance,
        payouts: CONFIG_PAYOUTS
          .range(deps.storage, None, None, Order::Ascending)
          .map(|r| r.unwrap().1)
          .collect(),
      }))
    })?,

    tax_rate: loader.view("tax_rate", || {
      Ok(Some(
        TAXES
          .range(deps.storage, None, None, Order::Ascending)
          .map(|r| r.unwrap().1)
          .sum(),
      ))
    })?,

    account: loader.account_view("account", |addr| {
      let maybe_account = ACCOUNTS.may_load(deps.storage, addr.clone())?;
      if let Some(account) = maybe_account {
        let maybe_claim = match CLAIMS.may_load(deps.storage, addr.clone())? {
          Some(mut claim) => {
            let drawing = DRAWINGS.load(deps.storage, claim.round_no.into())?;
            let payouts = load_payouts(deps.storage).unwrap();
            claim.amount = Some(calc_total_claim_amount(&claim, &drawing, &payouts));
            claim.tickets = Some(load_claim_tickets_by_account(deps.storage, &addr)?);
            Some(claim)
          },
          None => None,
        };
        return Ok(Some(AccountView {
          totals: account.totals,
          claim: maybe_claim,
          tickets: ROUND_TICKETS
            .prefix(addr.clone())
            .range(deps.storage, None, None, Order::Ascending)
            .map(|r| r.unwrap().1)
            .collect(),
        }));
      } else {
        Ok(None)
      }
    })?,
  })
}

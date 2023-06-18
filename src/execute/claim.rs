use crate::{
  error::ContractError,
  state::{
    load_claim, load_drawing, load_payouts, require_active_game_state, ACCOUNTS, CLAIMS,
    CONFIG_TOKEN,
  },
  util::calc_total_claim_amount,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};
use cw_lib::utils::funds::build_send_submsg;

pub fn claim(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> Result<Response, ContractError> {
  require_active_game_state(deps.storage)?;

  let claim = load_claim(deps.storage, &info.sender)?;
  let drawing = load_drawing(deps.storage, claim.round_no)?;
  let payouts = load_payouts(deps.storage)?;
  let token = CONFIG_TOKEN.load(deps.storage)?;
  let claim_amount = calc_total_claim_amount(&claim, &drawing, &payouts);

  CLAIMS.remove(deps.storage, info.sender.clone());

  ACCOUNTS.update(
    deps.storage,
    info.sender.clone(),
    |maybe_account| -> Result<_, ContractError> {
      if let Some(mut account) = maybe_account {
        account.totals.winnings = claim_amount;
        account.totals.wins += claim.match_counts.iter().map(|x| *x as u32).sum::<u32>();
        Ok(account)
      } else {
        Err(ContractError::AccountNotFound)
      }
    },
  )?;

  let resp = Response::new().add_attributes(vec![attr("action", "claim")]);

  Ok(if claim_amount.is_zero() {
    resp
  } else {
    resp.add_submessage(build_send_submsg(&info.sender, claim_amount, &token)?)
  })
}

use crate::{
  error::ContractError,
  models::PlayerWin,
  state::{
    ACCOUNTS, CONFIG_MAX_NUMBER, CONFIG_NUMBER_COUNT, CONFIG_ROUND_SECONDS, CONFIG_TOKEN,
    LOOKUP_TABLE, MAX_RECENT_WINS_LEN, ROUND_COUNTER, ROUND_START, TAX_RATES, TICKETS,
    TICKET_COUNT,
  },
  util::hash_numbers,
};
use cosmwasm_std::{
  attr, Addr, BlockInfo, DepsMut, Env, MessageInfo, Order, Response, Storage, SubMsg, Uint128,
  Uint64,
};
use cw_lib::{
  models::Token,
  random::{Pcg64, RngComponent},
  utils::funds::{build_send_submsg, get_token_balance},
};

pub fn draw(
  deps: DepsMut,
  env: Env,
  _info: MessageInfo,
) -> Result<Response, ContractError> {
  authorize_draw(deps.storage, &env.block)?;

  let round_no = ROUND_COUNTER.load(deps.storage)?;
  let token = CONFIG_TOKEN.load(deps.storage)?;
  let balance = get_token_balance(deps.querier, &env.contract.address, &token)?;

  let mut resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  let (hash, winners) = draw_winning_addresses(deps.storage, &env)?;

  // Build SubMsgs that send rewards to winners and tax recipients.
  if !(balance.is_zero() || winners.is_empty()) {
    resp = resp.add_submessages(build_transfer_submsgs(
      deps.storage,
      &env.block,
      &token,
      balance,
      winners,
      round_no,
      &hash,
    )?);
  }

  reset_state_for_next_round(deps.storage)?;

  Ok(resp)
}

fn authorize_draw(
  storage: &dyn Storage,
  block: &BlockInfo,
) -> Result<(), ContractError> {
  let round_start = ROUND_START.load(storage)?;
  let round_duration = CONFIG_ROUND_SECONDS.load(storage)?;
  if (round_start.seconds() + round_duration.u64()) > block.time.seconds() {
    return Err(ContractError::ActiveRound);
  }
  Ok(())
}

fn reset_state_for_next_round(storage: &mut dyn Storage) -> Result<(), ContractError> {
  // Clean up last round's state and increment round counter.
  TICKETS.clear(storage);
  LOOKUP_TABLE.clear(storage);
  TICKET_COUNT.save(storage, &0)?;
  ROUND_COUNTER.update(storage, |n| -> Result<_, ContractError> {
    Ok(n + Uint64::one())
  })?;
  Ok(())
}

fn build_transfer_submsgs(
  storage: &mut dyn Storage,
  block: &BlockInfo,
  token: &Token,
  balance: Uint128,
  winners: Vec<Addr>,
  round_no: Uint64,
  winning_hash: &String,
) -> Result<Vec<SubMsg>, ContractError> {
  let mut balance_post_tax = balance.clone();
  let mut send_submsgs: Vec<SubMsg> = Vec::with_capacity(winners.len() + 5);

  // Build send SubMsgs for sending taxes
  for result in TAX_RATES.range(storage, None, None, Order::Ascending) {
    let (addr, pct) = result?;
    let amount = balance.multiply_ratio(pct, Uint128::from(1_000_000u128));
    if !amount.is_zero() {
      send_submsgs.push(build_send_submsg(&addr, amount, token)?);
      balance_post_tax -= amount;
    }
  }

  if winners.len() > 0 {
    // Compute balance amount per winner
    let win_amount = balance_post_tax / Uint128::from(winners.len() as u128);

    for winner_addr in winners.iter() {
      if !win_amount.is_zero() {
        // Build send SubMsgs for sending winners their rewards
        send_submsgs.push(build_send_submsg(&winner_addr, win_amount, token)?);
      }

      // Update player account totals
      ACCOUNTS.update(
        storage,
        winner_addr.clone(),
        |maybe_account| -> Result<_, ContractError> {
          if let Some(mut account) = maybe_account {
            account.win_count += 1;
            account.total_win_amount += win_amount;

            // add the win, ensuring that the recent_wins
            // vec doesn't grow beyond len 10.
            let current_wins = &account.recent_wins;
            if !current_wins.is_empty() {
              let n = current_wins.len();
              let mut new_wins = vec![PlayerWin {
                amount: win_amount,
                time: block.time,
                round_no: round_no,
                hash: winning_hash.clone(),
              }];
              new_wins.append(&mut current_wins[..n.min(MAX_RECENT_WINS_LEN - 1)].to_vec());
              account.recent_wins = new_wins;
            }

            Ok(account)
          } else {
            Err(ContractError::AccountNotFound)
          }
        },
      )?;
    }
  }

  Ok(send_submsgs)
}

fn draw_winning_addresses(
  storage: &dyn Storage,
  env: &Env,
) -> Result<(String, Vec<Addr>), ContractError> {
  let round_index = ROUND_COUNTER.load(storage)?;
  let winning_hash = build_winning_hash(storage, round_index, &env)?;
  Ok((
    winning_hash.clone(),
    LOOKUP_TABLE
      .prefix(winning_hash)
      .keys(storage, None, None, Order::Ascending)
      .map(|r| r.unwrap())
      .collect(),
  ))
}

fn build_winning_hash(
  storage: &dyn Storage,
  round_index: Uint64,
  env: &Env,
) -> Result<String, ContractError> {
  let number_count = CONFIG_NUMBER_COUNT.load(storage)?;
  let max_value = CONFIG_MAX_NUMBER.load(storage)?;
  let mut winning_numbers: Vec<u16> = Vec::with_capacity(number_count as usize);
  let mut rng = Pcg64::from_components(&vec![
    RngComponent::Str(env.contract.address.to_string()),
    RngComponent::Int(env.block.height),
    RngComponent::Int(round_index.u64()),
    RngComponent::Int(env.block.time.nanos()),
    RngComponent::Int(
      env
        .transaction
        .clone()
        .and_then(|t| Some(t.index as u64))
        .unwrap_or(0u64),
    ),
  ]);

  for _ in 0..number_count {
    winning_numbers.push((rng.next_u64() % (max_value as u64)) as u16);
  }

  Ok(hash_numbers(&winning_numbers))
}

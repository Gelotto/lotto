use std::{
  collections::{HashMap, HashSet},
  marker::PhantomData,
};

use crate::{
  error::ContractError,
  models::{Claim, Drawing, Payout, RoundStatus},
  state::{
    ensure_sender_is_allowed, load_drawing, load_payouts, load_winning_numbers, CLAIMS,
    CONFIG_MAX_NUMBER, CONFIG_NUMBER_COUNT, CONFIG_ROUND_SECONDS, CONFIG_TOKEN, DRAWINGS, ROUND_NO,
    ROUND_START, ROUND_STATUS, ROUND_TICKETS, ROUND_TICKET_COUNT, TAXES, WINNING_TICKETS,
  },
};
use cosmwasm_std::{
  attr, Addr, BlockInfo, DepsMut, Env, MessageInfo, Order, Response, Storage, SubMsg, Timestamp,
  Uint128, Uint64,
};
use cw_lib::{
  models::Token,
  random::{Pcg64, RngComponent},
  utils::funds::{build_send_submsg, get_token_balance},
};
use cw_storage_plus::Bound;

pub const TICKET_PAGE_SIZE: usize = 500;

pub fn draw(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
) -> Result<Response, ContractError> {
  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "draw")?;
  let round_no = ROUND_NO.load(deps.storage)?;
  match ROUND_STATUS.load(deps.storage)? {
    RoundStatus::Active => start_processing_tickets(deps, env, info, round_no),
    RoundStatus::Drawing => process_next_ticket_batch(deps, env, info, round_no),
  }
}

pub fn start_processing_tickets(
  deps: DepsMut,
  env: Env,
  _info: MessageInfo,
  round_no: Uint64,
) -> Result<Response, ContractError> {
  ensure_round_can_end(deps.storage, &env.block)?;

  // Get contract's token balance
  let token = CONFIG_TOKEN.load(deps.storage)?;
  let payouts = load_payouts(deps.storage)?;
  let total_ticket_count = ROUND_TICKET_COUNT.load(deps.storage)?;
  let mut resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  // No need to perform any draw logic if there aren't any tickets, so just end
  // the round and prepare for the next.
  if total_ticket_count == 0 {
    resp = resp.add_attribute("is_complete", true.to_string());
    end_draw(deps.storage, env.block.time)?;
    return Ok(resp);
  }

  // Get the current balance. After subtracting any taxes, we save this amount
  // as the total pot size to be divided up among winning tickets.
  let balance = get_token_balance(deps.querier, &env.contract.address, &token)?;

  // Compute total tax amount owed and append send messages to response for
  // sending tokens to each tax recipient.
  let post_tax_balance = if !balance.is_zero() {
    let (submsgs, tax_amount) = build_tax_send_submsgs(deps.storage, &token, balance)?;
    resp = resp.add_submessages(submsgs);
    balance - tax_amount
  } else {
    balance
  };

  // Select and save the winning numbers
  let winning_numbers = choose_winning_numbers(deps.storage, &env)?;

  // Init a Drawing record, which keeps track of the round's status with respect
  // to its drawing. This object aggregates totals accumulated across as many
  // transactions as it takes to complete the drawing process.
  let mut drawing = Drawing {
    total_ticket_count,
    total_balance: post_tax_balance,
    winning_numbers: winning_numbers.iter().map(|x| *x).collect(),
    processed_ticket_count: 0,
    match_counts: vec![],
    cursor: None,
  };

  // Process first page of tickets, updating the Drawing.
  process_next_page(
    deps.storage,
    &payouts,
    &winning_numbers,
    round_no,
    &mut drawing,
  )?;

  // If there's only one page worth of tickets, we can end the drawing process
  // now; otherwise, we toggle the game state to "drawing" until a subsequent
  // execution of the contract's draw function resets it to active, implying
  // that we've entered the next round.
  if drawing.is_complete() {
    end_draw(deps.storage, env.block.time)?;
  } else {
    ROUND_STATUS.save(deps.storage, &RoundStatus::Drawing)?;
  }

  // Persist accumulated changes to the Drawing
  DRAWINGS.save(deps.storage, round_no.into(), &drawing)?;

  Ok(resp.add_attribute("is_complete", drawing.is_complete().to_string()))
}

pub fn process_next_page(
  storage: &mut dyn Storage,
  payouts: &HashMap<u8, Payout>,
  winning_numbers: &HashSet<u16>,
  round_no: Uint64,
  drawing: &mut Drawing,
) -> Result<(), ContractError> {
  let min = if let Some(cursor) = &drawing.cursor {
    Some(Bound::Exclusive((cursor.clone(), PhantomData)))
  } else {
    None
  };

  // Total number of tickets processed in this call:
  let mut processed_ticket_count = 0;

  // In-memory cache of Claim records encountered once or more within the scope
  // of processing this batch of tickets.
  let mut claims: HashMap<Addr, Claim> = HashMap::with_capacity(8);

  // The last TICKETS Map key in the batch, used upon the next execution of draw
  // as a cursor (for pagination):
  let mut cursor: Option<(u64, Addr, String)> = None;

  // In-memory accumulator of winning tickets. These are saved back to the
  // WINNING_TICKETS Map at the end of this procedure:
  let mut winning_tickets: HashMap<(Addr, String), Vec<u16>> = HashMap::with_capacity(8);

  // This vec represents a frequency distribution, where each vec positional
  // index corresponds to a possible number of matching numbers that a ticket
  // can have. The value at each index is the number of times a ticket with this
  // number of matches was encountered:
  let mut match_counts: Vec<u16> = vec![0; winning_numbers.len() + 1];

  // Process each ticket in the batch...
  for result in ROUND_TICKETS
    .range(storage, min, None, Order::Ascending)
    .take(TICKET_PAGE_SIZE)
  {
    let ((round_no, addr, hash), numbers) = result?;

    // `n_matches` is the number of matching numbers contained in the ticket.
    let mut n_matches: u8 = 0;
    // Count num matching numbers in the ticket, incrementing `n_matches`
    for x in &numbers {
      if winning_numbers.contains(x) {
        n_matches += 1;
      }
    }

    // Update running batch-level totals & state:
    cursor = Some((round_no, addr.clone(), hash.clone()));
    match_counts[n_matches as usize] += 1;
    processed_ticket_count += 1;

    // Upsert a Claim record for this ticket's owner.
    let claim = {
      if claims.get(&addr).is_none() {
        let new_claim = Claim {
          round_no: round_no.into(),
          incentive: Uint128::zero(),
          match_counts: vec![0; winning_numbers.len() + 1],
        };
        claims.insert(addr.clone(), new_claim);
      };
      claims.get_mut(&addr).unwrap()
    };

    // Increment claim amount by base payout incentive.
    if let Some(payout) = payouts.get(&n_matches) {
      claim.incentive += payout.incentive;
      claim.match_counts[n_matches as usize] += 1;
    }

    // Save a record of the owner's winning ticket numbers
    winning_tickets.insert((addr, hash), numbers);
  }

  // Finally, save the accumulated winning tickets.
  for ((addr, hash), v) in winning_tickets.iter() {
    WINNING_TICKETS.save(storage, (round_no.into(), addr.clone(), hash.clone()), v)?;
  }

  // Save new or updated Claims.
  for (addr, claim) in claims.iter() {
    CLAIMS.save(storage, addr.clone(), claim)?;
  }

  // Update the current Drawing
  drawing.processed_ticket_count += processed_ticket_count;
  drawing.cursor = cursor;
  for (i, n) in match_counts.iter().enumerate() {
    drawing.match_counts[i] += n;
  }

  Ok(())
}

pub fn process_next_ticket_batch(
  deps: DepsMut,
  env: Env,
  _info: MessageInfo,
  round_no: Uint64,
) -> Result<Response, ContractError> {
  let payouts = load_payouts(deps.storage)?;
  let winning_numbers = load_winning_numbers(deps.storage, round_no.into())?;
  let mut drawing = load_drawing(deps.storage, round_no)?;

  // Process next "page" of tickets, updating the Drawing and Claim records.
  process_next_page(
    deps.storage,
    &payouts,
    &winning_numbers,
    round_no,
    &mut drawing,
  )?;

  // Reset contract state for next round.
  if drawing.is_complete() {
    end_draw(deps.storage, env.block.time)?;
  }

  // Save accumulated state changes to the Drawing
  DRAWINGS.save(deps.storage, round_no.into(), &drawing)?;

  Ok(Response::new().add_attributes(vec![
    attr("action", "draw"),
    attr("is_complete", drawing.is_complete().to_string()),
  ]))
}

/// Clean up last round's state and increment round counter.
pub fn end_draw(
  storage: &mut dyn Storage,
  time: Timestamp,
) -> Result<(), ContractError> {
  ROUND_STATUS.save(storage, &RoundStatus::Active)?;
  ROUND_TICKETS.clear(storage);
  ROUND_START.save(storage, &time)?;
  ROUND_TICKET_COUNT.save(storage, &0)?;
  ROUND_NO.update(storage, |n| -> Result<_, ContractError> {
    Ok(n + Uint64::one())
  })?;
  Ok(())
}

///
fn ensure_round_can_end(
  storage: &dyn Storage,
  block: &BlockInfo,
) -> Result<(), ContractError> {
  if RoundStatus::Active == ROUND_STATUS.load(storage)? {
    let round_start = ROUND_START.load(storage)?;
    let round_duration = CONFIG_ROUND_SECONDS.load(storage)?;

    // Abort if the round hasn't reach its end time
    if (round_start.seconds() + round_duration.u64()) > block.time.seconds() {
      return Err(ContractError::ActiveRound);
    }
  }
  Ok(())
}

fn build_tax_send_submsgs(
  storage: &mut dyn Storage,
  token: &Token,
  balance: Uint128,
) -> Result<(Vec<SubMsg>, Uint128), ContractError> {
  // Build send SubMsgs for sending taxes
  let mut send_submsgs: Vec<SubMsg> = Vec::with_capacity(5);
  let mut total_amount = Uint128::zero();
  for result in TAXES.range(storage, None, None, Order::Ascending) {
    let (addr, pct) = result?;
    let amount = balance.multiply_ratio(pct, Uint128::from(1_000_000u128));
    if !amount.is_zero() {
      send_submsgs.push(build_send_submsg(&addr, amount, token)?);
      total_amount += amount;
    }
  }
  Ok((send_submsgs, total_amount))
}

fn choose_winning_numbers(
  storage: &mut dyn Storage,
  env: &Env,
) -> Result<HashSet<u16>, ContractError> {
  let round_no = ROUND_NO.load(storage)?;
  let numbers = compute_winning_numbers(storage, round_no, &env)?;
  Ok(HashSet::from_iter(numbers.iter().map(|x| *x)))
}

fn compute_winning_numbers(
  storage: &dyn Storage,
  round_no: Uint64,
  env: &Env,
) -> Result<Vec<u16>, ContractError> {
  let number_count = CONFIG_NUMBER_COUNT.load(storage)?;
  let max_value = CONFIG_MAX_NUMBER.load(storage)?;
  let mut winning_numbers: HashSet<u16> = HashSet::with_capacity(number_count as usize);
  let mut rng = Pcg64::from_components(&vec![
    RngComponent::Str(env.contract.address.to_string()),
    RngComponent::Int(env.block.height),
    RngComponent::Int(round_no.u64()),
    RngComponent::Int(env.block.time.nanos()),
    RngComponent::Int(
      env
        .transaction
        .clone()
        .and_then(|t| Some(t.index as u64))
        .unwrap_or(0u64),
    ),
  ]);

  while winning_numbers.len() < number_count as usize {
    winning_numbers.insert((rng.next_u64() % (max_value as u64)) as u16);
  }

  Ok(winning_numbers.iter().map(|x| *x).collect())
}

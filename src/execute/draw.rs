use std::{
  collections::{HashMap, HashSet},
  marker::PhantomData,
};

use crate::{
  error::ContractError,
  models::{Claim, Drawing, Payout, RoundStatus},
  state::{
    load_drawing, load_payouts, load_tickets_by_account, load_winning_numbers, BALANCE_CLAIMABLE,
    CLAIMS, CONFIG_DRAWER, CONFIG_HOUSE_ADDR, CONFIG_MARKETING, CONFIG_MAX_NUMBER,
    CONFIG_MIN_BALANCE, CONFIG_NUMBER_COUNT, CONFIG_PAYOUTS, CONFIG_PRICE, CONFIG_ROLLING,
    CONFIG_ROUND_SECONDS, CONFIG_STYLE, CONFIG_TOKEN, DEBUG_WINNING_NUMBERS, DRAWINGS,
    HOUSE_POT_TAX_PCT, ROUND_NO, ROUND_START, ROUND_STATUS, ROUND_TICKETS, ROUND_TICKET_COUNT,
    STAGED_CONFIG,
  },
  util::mul_pct,
};
use cosmwasm_std::{
  attr, Addr, Api, BlockInfo, Coin, DepsMut, Env, MessageInfo, Order, Response, Storage, Uint128,
  Uint64, WasmMsg,
};
use cw_lib::{
  models::Token,
  random::{Pcg64, RngComponent},
  utils::funds::get_token_balance,
};
use cw_storage_plus::Bound;
use house_staking::{client::House, models::AccountTokenAmount};

pub const TICKET_PAGE_SIZE: usize = 1000;

pub fn draw(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
) -> Result<Response, ContractError> {
  if info.sender != CONFIG_DRAWER.load(deps.storage)? {
    return Err(ContractError::NotAuthorized);
  }
  let round_no = ROUND_NO.load(deps.storage)?;
  match ROUND_STATUS.load(deps.storage)? {
    RoundStatus::Active => start_processing_tickets(deps, env, info, round_no),
    RoundStatus::Drawing => process_next_ticket_batch(deps, env, info, round_no),
  }
}

pub fn start_processing_tickets(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  round_no: Uint64,
) -> Result<Response, ContractError> {
  ensure_round_can_end(deps.storage, &env.block)?;

  // Get contract's token balance
  let token = CONFIG_TOKEN.load(deps.storage)?;
  let payouts = load_payouts(deps.storage)?;
  let ticket_count = ROUND_TICKET_COUNT.load(deps.storage)?;
  let mut resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  deps
    .api
    .debug(format!(">>> ticket count: {}", ticket_count).as_str());

  // No need to perform any draw logic if there aren't any tickets, so just end
  // the round and prepare for the next.
  if ticket_count == 0 {
    resp = resp.add_attribute("is_complete", true.to_string());
    reset_round_state(deps.storage, &env, ticket_count)?;
    return Ok(resp);
  }

  // Get the current balance. After subtracting any taxes, we save this amount
  // as the total pot size to be divided up among winning tickets.
  let contract_balance = get_token_balance(deps.querier, &env.contract.address, &token)?;

  deps
    .api
    .debug(format!(">>> balance: {}", contract_balance.u128()).as_str());

  let taxable_balance = contract_balance - BALANCE_CLAIMABLE.load(deps.storage)?;

  deps
    .api
    .debug(format!(">>> taxable balance: {}", taxable_balance.u128()).as_str());

  // Select and save the winning numbers
  let winning_numbers = choose_winning_numbers(deps.storage, &env)?;

  deps
    .api
    .debug(format!(">>> winning numbers: {:?}", winning_numbers).as_str());

  // Init a Drawing record, which keeps track of the round's status with respect
  // to its drawing. This object aggregates totals accumulated across as many
  // transactions as it takes to complete the drawing process.
  let mut drawing = Drawing {
    ticket_count,
    round_balance: taxable_balance,
    start_balance: CONFIG_MIN_BALANCE.load(deps.storage)?,
    winning_numbers: winning_numbers.iter().map(|x| *x).collect(),
    match_counts: vec![0; winning_numbers.len() + 1],
    processed_ticket_count: 0,
    total_payout: Uint128::zero(),
    pot_payout: Uint128::zero(),
    incentive_payout: Uint128::zero(),
    cursor: None,
    round_no: None,
  };

  deps
    .api
    .debug(format!(">>> calling process_next_page").as_str());

  // Process first page of tickets, updating the Drawing.
  process_next_page(
    deps.storage,
    deps.api,
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
    deps.api.debug(format!(">>> calling end_draw").as_str());
    if let Some(house_msgs) = end_draw(
      deps.storage,
      deps.api,
      &env,
      &info.funds,
      &payouts,
      &mut drawing,
    )? {
      resp = resp.add_messages(house_msgs);
    }
  } else {
    ROUND_STATUS.save(deps.storage, &RoundStatus::Drawing)?;
  }

  // Persist accumulated changes to the Drawing
  DRAWINGS.save(deps.storage, round_no.into(), &drawing)?;

  Ok(resp.add_attribute("is_complete", drawing.is_complete().to_string()))
}

pub fn process_next_page(
  storage: &mut dyn Storage,
  api: &dyn Api,
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
  let mut processed_ticket_count: u32 = 0;

  // In-memory cache of Claim records encountered once or more within the scope
  // of processing this batch of tickets.
  let mut claims: HashMap<Addr, Claim> = HashMap::with_capacity(8);

  // The last TICKETS Map key in the batch, used upon the next execution of draw
  // as a cursor (for pagination):
  let mut cursor: Option<(Addr, String)> = None;

  // This vec represents a frequency distribution, where each vec positional
  // index corresponds to a possible number of matching numbers that a ticket
  // can have. The value at each index is the number of times a ticket with this
  // number of matches was encountered:
  let mut match_counts: Vec<u16> = vec![0; winning_numbers.len() + 1];

  api.debug(format!(">>> initialized match_counts: {:?}", match_counts).as_str());

  // Process each ticket in the batch...
  for result in ROUND_TICKETS
    .range(storage, min, None, Order::Ascending)
    .take(TICKET_PAGE_SIZE)
  {
    let ((addr, hash), ticket) = result?;

    // `n_matches` is the number of matching numbers contained in the ticket.
    let mut n_matching_numbers: u8 = 0;

    // Count num matching numbers in the ticket, incrementing `n_matches`
    for x in &ticket.numbers {
      if winning_numbers.contains(x) {
        n_matching_numbers += 1;
      }
    }

    api.debug(format!(">>> n_matching_numbers: {:?}", n_matching_numbers).as_str());

    // Update running batch-level totals & state:
    cursor = Some((addr.clone(), hash.clone()));
    match_counts[n_matching_numbers as usize] += ticket.n;
    processed_ticket_count += ticket.n as u32;

    // Upsert the account's claim record with updated match counts
    if let Some(_) = payouts.get(&n_matching_numbers) {
      let claim: &mut Claim = {
        if claims.get(&addr).is_none() {
          let new_claim = Claim {
            round_no: round_no.into(),
            tickets: load_tickets_by_account(storage, &addr)?,
            matches: vec![0; winning_numbers.len() + 1],
            amount: None,
          };
          claims.insert(addr.clone(), new_claim);
        };
        claims.get_mut(&addr).unwrap()
      };

      claim.matches[n_matching_numbers as usize] += ticket.n;
    }
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
  info: MessageInfo,
  round_no: Uint64,
) -> Result<Response, ContractError> {
  let payouts = load_payouts(deps.storage)?;
  let winning_numbers = load_winning_numbers(deps.storage, round_no.into())?;
  let mut drawing = load_drawing(deps.storage, round_no)?;

  // Process next "page" of tickets, updating the Drawing and Claim records.
  process_next_page(
    deps.storage,
    deps.api,
    &payouts,
    &winning_numbers,
    round_no,
    &mut drawing,
  )?;

  let mut resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  // Reset contract state for next round.
  if drawing.is_complete() {
    if let Some(house_msgs) = end_draw(
      deps.storage,
      deps.api,
      &env,
      &info.funds,
      &payouts,
      &mut drawing,
    )? {
      resp = resp.add_messages(house_msgs);
    }
  }

  // Save accumulated state changes to the Drawing
  DRAWINGS.save(deps.storage, round_no.into(), &drawing)?;

  Ok(resp.add_attribute("is_complete", drawing.is_complete().to_string()))
}

pub fn reset_round_state(
  storage: &mut dyn Storage,
  env: &Env,
  current_ticket_count: u32,
) -> Result<(), ContractError> {
  ROUND_STATUS.save(storage, &RoundStatus::Active)?;
  ROUND_START.save(storage, &env.block.time)?;
  ROUND_NO.update(storage, |n| -> Result<_, ContractError> {
    Ok(n + Uint64::one())
  })?;
  // only bother clearing round state data structures if any tickets were sold
  if current_ticket_count > 0 {
    ROUND_TICKETS.clear(storage);
    ROUND_TICKET_COUNT.save(storage, &0)?;
  }

  // If there is a new config staged, then we update the config vars here at the
  // end of the latest (this) draw. Note that we never update the TOKEN config
  // var, since this must remain constant for the claim and withdraw to continue
  // working.
  if let Some(new_config) = STAGED_CONFIG.load(storage)? {
    CONFIG_HOUSE_ADDR.save(storage, &new_config.house_address)?;
    CONFIG_MAX_NUMBER.save(storage, &new_config.max_number)?;
    CONFIG_MIN_BALANCE.save(storage, &new_config.min_balance)?;
    CONFIG_NUMBER_COUNT.save(storage, &new_config.number_count)?;
    CONFIG_ROLLING.save(storage, &new_config.rolling)?;
    CONFIG_ROUND_SECONDS.save(storage, &new_config.round_seconds)?;
    CONFIG_MARKETING.save(storage, &new_config.marketing)?;
    CONFIG_PRICE.save(storage, &new_config.price)?;
    CONFIG_STYLE.save(storage, &new_config.style)?;

    CONFIG_PAYOUTS.clear(storage);
    for payout in new_config.payouts {
      CONFIG_PAYOUTS.save(storage, payout.n, &payout)?;
    }

    // clear staged Config changes from state
    STAGED_CONFIG.save(storage, &None)?;
  }

  Ok(())
}

/// Clean up last round's state and increment round counter.
pub fn end_draw(
  storage: &mut dyn Storage,
  api: &dyn Api,
  env: &Env,
  funds: &Vec<Coin>,
  payouts: &HashMap<u8, Payout>,
  drawing: &mut Drawing,
) -> Result<Option<Vec<WasmMsg>>, ContractError> {
  let ticket_count = ROUND_TICKET_COUNT.load(storage)?;

  // If maybe_drawing is None, it means that there are no tickets, so we skip
  // the follow.
  //
  // Otherwise, we compute the total incentive needed for processing claims and
  // transfer it to this contract's balance from the house.
  // Compute total incentive amount required for pending claims
  let (incentive_payout_amount, taxable_pot_payout_amount) = {
    let mut incentive_amount = Uint128::zero();
    let mut pot_payout_amount = Uint128::zero();

    let pot_size = drawing.resolve_pot_size(); // pre-tax amount

    for (n_matches, payout) in payouts.iter() {
      let n_tickets = drawing.match_counts[(*n_matches) as usize];
      if n_tickets > 0 {
        // increment payout amount by incentive
        if !payout.incentive.is_zero() {
          incentive_amount += payout.incentive * Uint128::from(n_tickets);
        }
        if !payout.pct.is_zero() {
          pot_payout_amount += mul_pct(pot_size, payout.pct);
        }
      }
    }
    (incentive_amount, pot_payout_amount)
  };

  // Compute total tax amount owed and append send messages to response for
  // sending tokens to each tax recipient.
  let tax_amount = if !taxable_pot_payout_amount.is_zero() {
    mul_pct(taxable_pot_payout_amount, HOUSE_POT_TAX_PCT.into())
  } else {
    Uint128::zero()
  };

  api.debug(
    format!(
      ">>> pot payout amount: {}",
      taxable_pot_payout_amount.u128()
    )
    .as_str(),
  );
  api.debug(format!(">>> pot tax amount: {}", tax_amount.u128()).as_str());

  // Set drawing total values
  drawing.pot_payout = taxable_pot_payout_amount - tax_amount;
  drawing.incentive_payout = incentive_payout_amount;
  drawing.total_payout = drawing.incentive_payout + drawing.pot_payout; // TODO: Deprecate this variable
  drawing.cursor = None;

  BALANCE_CLAIMABLE.update(storage, |total| -> Result<_, ContractError> {
    Ok(total + drawing.total_payout)
  })?;

  // Get CW20 address for house process msg
  let token = CONFIG_TOKEN.load(storage)?;
  let maybe_token_addr = if let Token::Cw20 { address } = token {
    Some(address)
  } else {
    None
  };

  // Build message to take incentives from house
  let is_rolling = CONFIG_ROLLING.load(storage)?;
  let taxed_payout = drawing.resolve_total_payout(); // tax already deducted

  let total_outgoing = taxed_payout;

  let total_incoming: Uint128 = if !is_rolling {
    drawing.round_balance
  } else if drawing.round_balance >= taxed_payout + tax_amount {
    taxed_payout + tax_amount
  } else {
    drawing.round_balance
  };

  reset_round_state(storage, env, ticket_count)?;

  api.debug(format!(">>> {:?}", drawing).as_str());
  api.debug(format!(">>> house incoming : {:?}", total_incoming.u128()).as_str());
  api.debug(format!(">>> house outgoing: {:?}", total_outgoing.u128()).as_str());

  let house = House::new(&CONFIG_HOUSE_ADDR.load(storage)?);

  Ok(Some(house.process(
    env.contract.address.clone(),
    // Incoming to house:
    Some(AccountTokenAmount::new(
      &env.contract.address,
      total_incoming,
    )),
    // Outgoing from house:
    Some(AccountTokenAmount::new(
      &env.contract.address,
      total_outgoing,
    )),
    Some(funds.clone()),
    maybe_token_addr,
  )?))
}

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
  if let Some(debug_winning_numbers) = DEBUG_WINNING_NUMBERS.load(storage)? {
    return Ok(debug_winning_numbers);
  }

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

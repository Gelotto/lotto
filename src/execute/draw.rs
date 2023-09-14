use std::{
  collections::{HashMap, HashSet},
  marker::PhantomData,
};

use crate::{
  error::ContractError,
  models::{Claim, Drawing, Payout, RoundStatus, Ticket},
  state::{
    draw_winning_numbers, load_drawing, load_payouts, load_winning_numbers, BALANCE_CLAIMABLE,
    CLAIMS, CONFIG_DRAWER, CONFIG_HOUSE_ADDR, CONFIG_MAX_NUMBER, CONFIG_MIN_BALANCE,
    CONFIG_NOIS_PROXY, CONFIG_NUMBER_COUNT, CONFIG_PAYOUTS, CONFIG_PRICE, CONFIG_ROLLING,
    CONFIG_ROUND_SECONDS, CONFIG_TICKET_BATCH_SIZE, CONFIG_TOKEN, DRAWINGS, HOUSE_POT_TAX_PCT,
    JACKPOT_CLAIMANTS, ROUND_NO, ROUND_START, ROUND_STATUS, ROUND_TICKETS, ROUND_TICKET_COUNT,
    STAGED_CONFIG,
  },
  util::mul_pct,
};
use cosmwasm_std::{
  attr, to_binary, Addr, Api, BlockInfo, Coin, DepsMut, Env, MessageInfo, Order, Response, Storage,
  Uint128, Uint64, WasmMsg,
};
use cw_lib::{models::Token, utils::funds::get_token_balance};
use cw_storage_plus::{Bound, Map};
use house_staking::{client::House, models::AccountTokenAmount};
use nois::{NoisCallback, ProxyExecuteMsg};

pub fn draw(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  maybe_callback: Option<NoisCallback>,
) -> Result<Response, ContractError> {
  let round_no = ROUND_NO.load(deps.storage)?;
  let round_status = ROUND_STATUS.load(deps.storage)?;

  // Are we using Nois?
  if let Some(proxy_addr) = CONFIG_NOIS_PROXY.may_load(deps.storage)?.unwrap_or(None) {
    match round_status {
      RoundStatus::Active => {
        // Gelotto backend requesting randomness. Lotto goes to Drawing state.
        if info.sender != CONFIG_DRAWER.load(deps.storage)? {
          return Err(ContractError::NotAuthorized);
        }
        ensure_round_can_end(deps.storage, &env.block)?;
        return request_randomness_from_nois(deps, env, info, round_no, proxy_addr);
      },
      RoundStatus::Drawing => {
        // If a Drawing struct exists, continue processing the next ticket batch.
        if let Some(mut drawing) = load_drawing(deps.storage, round_no).ok() {
          if info.sender != CONFIG_DRAWER.load(deps.storage)? {
            return Err(ContractError::NotAuthorized);
          }
          return process_next_ticket_batch(deps, env, info, round_no, &mut drawing);
        }
        // Otherwise, expect the next tx to receive randomness from nois proxy...
        else {
          if let Some(cb) = maybe_callback {
            // We've received randomness from Nois. The lotto is expected to be in the
            // Drawing state following a previous request_randomness tx.
            if info.sender != proxy_addr {
              return Err(ContractError::NotAuthorized);
            }
            return init_drawing_with_nois(deps, env, round_no, cb);
          } else {
            return Err(ContractError::WaitingForNois);
          }
        }
      },
    }
  }
  // We're using the built-in PRNG instead of Nois for E2E testing:
  else {
    match round_status {
      RoundStatus::Active => {
        ensure_round_can_end(deps.storage, &env.block)?;
        return start_processing_tickets(deps, env, info, round_no);
      },
      RoundStatus::Drawing => {
        let mut drawing = load_drawing(deps.storage, round_no)?;
        return process_next_ticket_batch(deps, env, info, round_no, &mut drawing);
      },
    }
  }
}

fn request_randomness_from_nois(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  round_no: Uint64,
  nois_proxy_addr: Addr,
) -> Result<Response, ContractError> {
  let ticket_count = ROUND_TICKET_COUNT.load(deps.storage)?;
  let mut resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  // No need to perform any draw logic if there aren't any tickets, so just end
  // the round and prepare for the next.
  if ticket_count == 0 {
    resp = resp.add_attribute("is_complete", true.to_string());
    reset_round_state(deps.storage, &env)?;
    return Ok(resp);
  }

  ROUND_STATUS.save(deps.storage, &RoundStatus::Drawing)?;

  Ok(
    Response::new()
      .add_attributes(vec![attr("action", "draw")])
      .add_message(WasmMsg::Execute {
        contract_addr: nois_proxy_addr.into(),
        msg: to_binary(&ProxyExecuteMsg::GetNextRandomness {
          job_id: round_no.to_string(),
        })?,
        funds: info.funds,
      }),
  )
}

fn init_drawing_with_nois(
  deps: DepsMut,
  env: Env,
  round_no: Uint64,
  callback: NoisCallback,
) -> Result<Response, ContractError> {
  let resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  let ticket_count = ROUND_TICKET_COUNT.load(deps.storage)?;
  let token = CONFIG_TOKEN.load(deps.storage)?;
  let contract_balance = get_token_balance(deps.querier, &env.contract.address, &token)?;
  let taxable_balance = contract_balance - BALANCE_CLAIMABLE.load(deps.storage)?;
  let winning_numbers = draw_winning_numbers(deps.storage, &env, None, None, None, Some(callback))?;

  // Init a Drawing record, which keeps track of the round's status with respect
  // to its drawing. This object aggregates totals accumulated across as many
  // transactions as it takes to complete the drawing process.
  let drawing = Drawing {
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

  // Persist accumulated changes to the Drawing
  DRAWINGS.save(deps.storage, round_no.into(), &drawing)?;

  Ok(resp)
}

fn start_processing_tickets(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  round_no: Uint64,
) -> Result<Response, ContractError> {
  // Get contract's token balance
  let ticket_count = ROUND_TICKET_COUNT.load(deps.storage)?;
  let mut resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  // No need to perform any draw logic if there aren't any tickets, so just end
  // the round and prepare for the next.
  if ticket_count == 0 {
    resp = resp.add_attribute("is_complete", true.to_string());
    reset_round_state(deps.storage, &env)?;
    return Ok(resp);
  }

  let token = CONFIG_TOKEN.load(deps.storage)?;
  let payouts = load_payouts(deps.storage)?;

  // Get the current balance. After subtracting any taxes, we save this amount
  // as the total pot size to be divided up among winning tickets.
  let contract_balance = get_token_balance(deps.querier, &env.contract.address, &token)?;
  let taxable_balance = contract_balance - BALANCE_CLAIMABLE.load(deps.storage)?;
  let winning_numbers = draw_winning_numbers(deps.storage, &env, None, None, None, None)?;

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
    if let Some(house_msgs) = end_draw(
      deps.storage,
      deps.api,
      &env,
      &info.funds,
      &payouts,
      &mut drawing,
      contract_balance,
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

  let page_size = CONFIG_TICKET_BATCH_SIZE.load(storage)? as usize;

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

  // accumulator for winning tickets, saved to state below.
  let mut claim_tickets: HashMap<Addr, Vec<(String, Ticket)>> = HashMap::with_capacity(64);

  let mut jackpot_claimant_addrs: Vec<Addr> = vec![];

  api.debug(format!(">>> initialized match_counts: {:?}", match_counts).as_str());

  // Process each ticket in the batch...
  for result in ROUND_TICKETS
    .range(storage, min, None, Order::Ascending)
    .take(page_size)
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
            is_approved: false,
            round_no: round_no.into(),
            matches: vec![0; winning_numbers.len() + 1],
            tickets: None,
            amount: None,
          };
          claims.insert(addr.clone(), new_claim);
        };
        claims.get_mut(&addr).unwrap()
      };

      claim.matches[n_matching_numbers as usize] += ticket.n;

      if n_matching_numbers as usize == winning_numbers.len() {
        jackpot_claimant_addrs.push(addr.clone());
      }

      // Collect winning ticket into the account's "claim tickets" vec. These
      // are saved to state below, in a dynamic map associated with the ticket
      // holder's address.
      if let Some(tickets_vec) = claim_tickets.get_mut(&addr) {
        tickets_vec.push((hash, ticket))
      } else {
        claim_tickets.insert(addr, vec![(hash, ticket)]);
      }
    }
  }

  for addr in jackpot_claimant_addrs.iter() {
    JACKPOT_CLAIMANTS.save(storage, addr, &true)?;
  }

  // Save new or updated Claims.
  for (addr, claim) in claims.iter() {
    CLAIMS.save(storage, addr.clone(), claim)?;

    // Save winning tickets corresponding to the upserted Claims
    let map_tag = format!("claim_tickets_{}", addr.to_string());
    let map: Map<String, Ticket> = Map::new(map_tag.as_str());
    if let Some(tickets_vec) = claim_tickets.get(addr) {
      for (hash, ticket) in tickets_vec.iter() {
        map.save(storage, hash.clone(), &ticket)?;
      }
    }
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
  drawing: &mut Drawing,
) -> Result<Response, ContractError> {
  let payouts = load_payouts(deps.storage)?;
  let winning_numbers = load_winning_numbers(deps.storage, round_no.into())?;

  // Process next "page" of tickets, updating the Drawing and Claim records.
  process_next_page(
    deps.storage,
    deps.api,
    &payouts,
    &winning_numbers,
    round_no,
    drawing,
  )?;

  let mut resp = Response::new().add_attributes(vec![attr("action", "draw")]);

  // Reset contract state for next round.
  if drawing.is_complete() {
    let token = CONFIG_TOKEN.load(deps.storage)?;
    let contract_balance = get_token_balance(deps.querier, &env.contract.address, &token)?;
    if let Some(house_msgs) = end_draw(
      deps.storage,
      deps.api,
      &env,
      &info.funds,
      &payouts,
      drawing,
      contract_balance,
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
) -> Result<(), ContractError> {
  ROUND_STATUS.save(storage, &RoundStatus::Active)?;
  ROUND_START.save(storage, &env.block.time)?;
  ROUND_NO.update(storage, |n| -> Result<_, ContractError> {
    Ok(n + Uint64::one())
  })?;
  ROUND_TICKETS.clear(storage);
  ROUND_TICKET_COUNT.save(storage, &0)?;

  // If there is a new config staged, then we update the config vars here at the
  // end of the latest (this) draw. Note that we never update the TOKEN config
  // var, since this must remain constant for the claim and withdraw to continue
  // working.
  if let Some(new_config) = STAGED_CONFIG.load(storage)? {
    CONFIG_MAX_NUMBER.save(storage, &new_config.max_number)?;
    CONFIG_MIN_BALANCE.save(storage, &new_config.min_balance)?;
    CONFIG_NUMBER_COUNT.save(storage, &new_config.number_count)?;
    CONFIG_ROLLING.save(storage, &new_config.rolling)?;
    CONFIG_ROUND_SECONDS.save(storage, &new_config.round_seconds)?;
    CONFIG_PRICE.save(storage, &new_config.price)?;

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
  _contract_balance: Uint128,
) -> Result<Option<Vec<WasmMsg>>, ContractError> {
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
    // Not rolling means that the entire round's balance goes to the house
    // regardless of the payout.
    drawing.round_balance
  } else if drawing.round_balance >= taxed_payout + tax_amount {
    // If the round ended with a surplus balance, beyond the pre-taxed total
    // payout, we can safely send the house this net amount. Since the amount in
    // includes tax but the payout does not, this effectively pays only taxes to
    // the house.
    taxed_payout + tax_amount
  } else {
    // However, if there's not enough balance to cover the total payout, we send
    // the house whatever we've got. In this case (like non-rolling games), the
    // house will pay the difference, outgoing_amount -
    // incoming_amount.
    drawing.round_balance
  };

  reset_round_state(storage, env)?;

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

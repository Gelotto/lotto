use crate::error::ContractError;
use crate::msg::DryRunResponse;
use crate::state::{draw_winning_numbers, generate_random_tickets};
use cosmwasm_std::{Deps, Env, Timestamp, Uint64};

pub fn dry_run(
  deps: Deps,
  env: Env,
  seed: u32,
  ticket_count: u16,
  entropy: String,
  maybe_time: Option<Timestamp>,
  maybe_height: Option<Uint64>,
  maybe_tx_index: Option<Uint64>,
) -> Result<DryRunResponse, ContractError> {
  let maybe_height = maybe_height.and_then(|h| Some(h.u64()));
  let maybe_tx_index = maybe_tx_index.and_then(|i| Some(i.u64()));
  let tickets = generate_random_tickets(deps.storage, ticket_count, seed)?;
  let winning_numbers: Vec<u16> = Vec::from_iter(
    draw_winning_numbers(
      deps.storage,
      &env,
      &entropy,
      maybe_time,
      maybe_height,
      maybe_tx_index,
    )?
    .iter()
    .map(|x| *x),
  );

  let mut match_counts: Vec<u16> = vec![0; winning_numbers.len() + 1];
  for ticket in tickets.iter() {
    let mut n_matching_numbers: u8 = 0;
    for x in ticket.iter() {
      if winning_numbers.contains(x) {
        n_matching_numbers += 1;
      }
    }
    match_counts[n_matching_numbers as usize] += 1;
  }

  Ok(DryRunResponse {
    seed,
    entropy,
    winning_numbers,
    match_counts,
  })
}

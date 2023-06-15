use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
  #[error("{0}")]
  Std(#[from] StdError),

  #[error("NotAuthorized")]
  NotAuthorized,

  #[error("TicketExists")]
  TicketExists,

  #[error("InvalidNumberCount")]
  InvalidNumberCount,

  #[error("NumberOutOfBounds")]
  NumberOutOfBounds,

  #[error("InsufficientFunds")]
  InsufficientFunds,

  #[error("AccountNotFound")]
  AccountNotFound,

  #[error("ActiveRound")]
  ActiveRound,
}

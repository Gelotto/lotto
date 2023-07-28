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

  #[error("DuplicateNumber")]
  DuplicateNumber,

  #[error("NumberOutOfBounds")]
  NumberOutOfBounds,

  #[error("InsufficientFunds")]
  InsufficientFunds,

  #[error("AlreadyClaimed")]
  AlreadyClaimed,

  #[error("ClaimNotFound")]
  ClaimNotFound,

  #[error("AccountNotFound")]
  AccountNotFound,

  #[error("DrawingNotFound")]
  DrawingNotFound,

  #[error("ActiveRound")]
  ActiveRound,

  #[error("InvalidRoundNo")]
  InvalidRoundNo,

  #[error("InvalidGameState")]
  InvalidGameState,

  #[error("ValidationError")]
  ValidationError,
}

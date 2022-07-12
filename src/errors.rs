use crate::engine::{ClientId, TxId};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub enum AccountingError {
    TransactionAlreadyExists(TxId),
    InsufficientFunds(ClientId),
    TransactionDoesntExist(TxId),
    TransactionIsAlreadyLocked(TxId),
    TransactionIsNotDisputed(TxId),
    AccountFrozen(ClientId),
    TransactionDoesntBelongToClient { tx_id: TxId, client: ClientId },
    InvalidAmount,
}

impl Display for AccountingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for AccountingError {}

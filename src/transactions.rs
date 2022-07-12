use crate::engine::{Amount, ClientId, TxId};

#[derive(Clone, Debug)]
pub enum Event {
    Deposit(Transaction),
    Withdrawal(Transaction),
    Dispute { client: ClientId, tx_id: TxId },
    Resolve { client: ClientId, tx_id: TxId },
    Chargeback { client: ClientId, tx_id: TxId },
}

#[derive(Clone, Debug)]
pub struct Transaction {
    pub id: TxId,
    pub client: ClientId,
    pub amount: Amount,
    pub is_locked: bool,
}

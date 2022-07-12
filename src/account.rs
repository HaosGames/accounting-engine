use crate::engine::{Amount, ClientId, TxId};
use crate::errors::AccountingError;
use crate::transactions::{Event, Transaction};
use std::collections::BTreeMap;
use std::sync::Arc;
use rust_decimal_macros::dec;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::RwLock;

pub struct Account {
    pub id: ClientId,
    pub available: Amount,
    pub held: Amount,
    pub is_locked: bool,
    pub incoming_tx: UnboundedReceiver<Event>,
    pub transactions: Arc<RwLock<BTreeMap<TxId, Transaction>>>,
}
impl Account {
    pub async fn process_txs(mut self) -> Self {
        loop {
            if let Some(tx) = self.incoming_tx.recv().await {
                if let Err(_e) = self.handle_tx(tx).await {
                    // eprintln!("{:?}", e);
                }
            } else {
                break;
            }
        }
        self
    }
    async fn try_insert_tx(&mut self, tx_id: TxId, tx: Transaction) -> Result<(), AccountingError> {
        let mut transactions = self.transactions.write().await;
        if transactions.contains_key(&tx_id) {
            return Err(AccountingError::TransactionAlreadyExists(tx_id));
        } else {
            transactions.insert(tx_id, tx);
        }
        Ok(())
    }
    async fn handle_tx(&mut self, tx: Event) -> Result<(), AccountingError> {
        if self.is_locked {
            return Err(AccountingError::AccountFrozen(self.id));
        }
        match tx {
            Event::Deposit(tx) => {
                self.try_insert_tx(tx.id, tx.clone()).await?;
                if tx.amount <= dec!(0) {
                    return Err(AccountingError::InvalidAmount);
                }
                if tx.is_locked {
                    self.held += tx.amount;
                } else {
                    self.available += tx.amount;
                }
            }
            Event::Withdrawal(mut tx) => {
                if self.available < tx.amount {
                    return Err(AccountingError::InsufficientFunds(self.id));
                }
                if tx.amount <= dec!(0) {
                    return Err(AccountingError::InvalidAmount);
                }
                tx.amount *= dec!(-1); // Invert amount to reflect the withdrawal in the tx catalog
                self.try_insert_tx(tx.id, tx.clone()).await?;
                self.available += tx.amount;
            }
            Event::Dispute { tx_id, .. } => {
                if let Some(to_lock_tx) = self.transactions.write().await.get_mut(&tx_id) {
                    if to_lock_tx.is_locked {
                        return Err(AccountingError::TransactionIsAlreadyLocked(tx_id));
                    }
                    if to_lock_tx.client != self.id {
                        return Err(AccountingError::TransactionDoesntBelongToClient {
                            tx_id,
                            client: self.id,
                        });
                    }
                    to_lock_tx.is_locked = true;
                    self.available -= to_lock_tx.amount;
                    self.held += to_lock_tx.amount;
                } else {
                    return Err(AccountingError::TransactionDoesntExist(tx_id));
                }
            }
            Event::Resolve { tx_id, .. } => {
                if let Some(to_lock_tx) = self.transactions.write().await.get_mut(&tx_id) {
                    if !to_lock_tx.is_locked {
                        return Err(AccountingError::TransactionIsNotDisputed(tx_id));
                    }
                    if to_lock_tx.client != self.id {
                        return Err(AccountingError::TransactionDoesntBelongToClient {
                            tx_id,
                            client: self.id,
                        });
                    }
                    to_lock_tx.is_locked = false;
                    self.available += to_lock_tx.amount;
                    self.held -= to_lock_tx.amount;
                } else {
                    return Err(AccountingError::TransactionDoesntExist(tx_id));
                }
            }
            Event::Chargeback { tx_id, .. } => {
                if let Some(to_lock_tx) = self.transactions.read().await.get(&tx_id) {
                    if !to_lock_tx.is_locked {
                        return Err(AccountingError::TransactionIsNotDisputed(tx_id));
                    }
                    if to_lock_tx.client != self.id {
                        return Err(AccountingError::TransactionDoesntBelongToClient {
                            tx_id,
                            client: self.id,
                        });
                    }
                    self.held -= to_lock_tx.amount;
                    self.is_locked = true;
                } else {
                    return Err(AccountingError::TransactionDoesntExist(tx_id));
                }
                self.transactions.write().await.remove(&tx_id);
            }
        }
        Ok(())
    }
}

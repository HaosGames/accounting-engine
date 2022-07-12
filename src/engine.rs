use crate::account::Account;
use crate::transactions::{Event, Transaction};
use std::collections::BTreeMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;

pub type ClientId = u16;
pub type TxId = u32;
pub type Amount = Decimal;

pub struct AccountingEngine {
    incoming_tx: UnboundedReceiver<Event>,
    transactions: Arc<RwLock<BTreeMap<TxId, Transaction>>>,
    tx_to_accounts: BTreeMap<ClientId, UnboundedSender<Event>>,
    result: Vec<JoinHandle<Account>>,
}
impl AccountingEngine {
    pub fn new() -> (Self, UnboundedSender<Event>) {
        let (sender, receiver) = unbounded_channel();
        (
            AccountingEngine {
                incoming_tx: receiver,
                transactions: Arc::new(Default::default()),
                tx_to_accounts: Default::default(),
                result: vec![],
            },
            sender,
        )
    }
    pub async fn process_txs(mut self) -> BTreeMap<ClientId, AccountingResult> {
        loop {
            if let Some(tx) = self.incoming_tx.recv().await {
                if let Err(_e) = self.handle_tx(tx).await {
                    // eprintln!("{:?}", e);
                }
            } else {
                break;
            }
        }
        self.tx_to_accounts = Default::default();
        let mut result = BTreeMap::default();
        for handle in self.result {
            if let Ok(account) = handle.await {
                result.insert(
                    account.id,
                    AccountingResult {
                        available: account.available,
                        held: account.held,
                        total: account.available + account.held,
                        locked: account.is_locked,
                    },
                );
            } else {
                // eprintln!("there was an error awaiting the account join handles");
            }
        }
        result
    }
    async fn handle_tx(&mut self, tx: Event) -> Result<(), Box<dyn Error>> {
        let client = match tx.clone() {
            Event::Deposit(tx) => tx.client,
            Event::Withdrawal(tx) => tx.client,
            Event::Dispute { client, .. } => client,
            Event::Resolve { client, .. } => client,
            Event::Chargeback { client, .. } => client,
        };
        if let Some(sender) = self.tx_to_accounts.get(&client) {
            sender.send(tx)?;
        } else {
            let (sender, receiver) = unbounded_channel();
            let account = Account {
                id: client,
                available: dec!(0),
                held: dec!(0),
                is_locked: false,
                incoming_tx: receiver,
                transactions: self.transactions.clone(),
            };
            sender.send(tx)?;
            self.tx_to_accounts.insert(client, sender);
            let account = tokio::spawn(async move { account.process_txs().await });
            self.result.push(account);
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq)]
pub struct AccountingResult {
    pub available: Amount,
    pub held: Amount,
    pub total: Amount,
    pub locked: bool,
}

#[cfg(test)]
#[allow(unused)]
mod test {
    use rust_decimal_macros::dec;
    use crate::engine::{AccountingEngine, AccountingResult};
    use crate::transactions::{Event, Transaction};

    #[tokio::test]
    async fn one_client_deposits() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn two_clients_deposit() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Deposit(Transaction {
                id: 1,
                client: 1,
                amount: dec!(2),
                is_locked: false,
            }))
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: false
            },
            result.get(&0).unwrap()
        );
        assert_eq!(
            &AccountingResult {
                available: dec!(2),
                held: dec!(0),
                total: dec!(2),
                locked: false
            },
            result.get(&1).unwrap()
        );
    }
    #[tokio::test]
    async fn one_client_deposits_and_withdrawals() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Withdrawal(Transaction {
                client: 0,
                id: 1,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(0),
                held: dec!(0),
                total: dec!(0),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn two_clients_deposit_and_withdrawal() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Withdrawal(Transaction {
                id: 1,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Deposit(Transaction {
                id: 2,
                client: 1,
                amount: dec!(2),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Withdrawal(Transaction {
                id: 3,
                client: 1,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(0),
                held: dec!(0),
                total: dec!(0),
                locked: false
            },
            result.get(&0).unwrap()
        );
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: false
            },
            result.get(&1).unwrap()
        );
    }
    #[tokio::test]
    async fn one_client_charges_back() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();

        sender
            .send(Event::Dispute {
                client: 0,
                tx_id: 0,
            })
            .unwrap();
        sender
            .send(Event::Chargeback {
                client: 0,
                tx_id: 0,
            })
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(0),
                held: dec!(0),
                total: dec!(0),
                locked: true,
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn one_client_resolves_dispute() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();

        sender
            .send(Event::Dispute {
                client: 0,
                tx_id: 0,
            })
            .unwrap();
        sender
            .send(Event::Resolve {
                client: 0,
                tx_id: 0,
            })
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: false,
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn dispute_non_existent_tx() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Dispute {
                client: 0,
                tx_id: 1
            })
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn resolve_non_locket_tx() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Resolve {
                client: 0,
                tx_id: 0
            })
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn dispute_locked_tx() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Dispute {
                client: 0,
                tx_id: 0
            })
            .unwrap();
        sender
            .send(Event::Dispute {
                client: 0,
                tx_id: 0
            })
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(0),
                held: dec!(1),
                total: dec!(1),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn deposit_locked_tx() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: true,
            }))
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(0),
                held: dec!(1),
                total: dec!(1),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn withdraw_more_than_deposited() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Withdrawal(Transaction {
                id: 1,
                client: 0,
                amount: dec!(2),
                is_locked: false,
            }))
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn charge_back_on_zero_balance() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Withdrawal(Transaction {
                id: 1,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender.send(Event::Dispute { client: 0, tx_id: 0 }).unwrap();
        sender.send(Event::Chargeback { client: 0, tx_id: 0 }).unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                //TODO Should a chargeback on zero balance result in negative balance?
                available: dec!(-1),
                held: dec!(0),
                total: dec!(-1),
                locked: true
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn charge_back_a_withdrawal() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Withdrawal(Transaction {
                id: 1,
                client: 0,
                amount: dec!(1),
                is_locked: false,
            }))
            .unwrap();
        sender.send(Event::Dispute { client: 0, tx_id: 1 }).unwrap();
        sender.send(Event::Chargeback { client: 0, tx_id: 1 }).unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(1),
                held: dec!(0),
                total: dec!(1),
                locked: true,
            },
            result.get(&0).unwrap()
        );
    }
    #[tokio::test]
    async fn deposits_and_dispute() {
        let (engine, sender) = AccountingEngine::new();
        sender
            .send(Event::Deposit(Transaction {
                id: 0,
                client: 0,
                amount: dec!(1.1),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Deposit(Transaction {
                id: 1,
                client: 0,
                amount: dec!(200.4567),
                is_locked: false,
            }))
            .unwrap();
        sender
            .send(Event::Dispute { client: 0, tx_id: 0 })
            .unwrap();
        drop(sender);
        let result = engine.process_txs().await;
        assert_eq!(
            &AccountingResult {
                available: dec!(200.4567),
                held: dec!(1.1),
                total: dec!(201.5567),
                locked: false
            },
            result.get(&0).unwrap()
        );
    }
}

use std::collections::BTreeMap;
use crate::engine::{AccountingEngine, AccountingResult, Amount, ClientId, TxId};
use crate::transactions::{Event, Transaction};
use std::error::Error;

mod account;
mod engine;
mod errors;
mod transactions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Some(input_path) = std::env::args().nth(1) {
        match csv::ReaderBuilder::new().trim(csv::Trim::All).from_path(input_path) {
            Ok(mut reader) => {
                let (engine, sender) = AccountingEngine::new();
                for entry in reader.deserialize() {
                    let record: Input = entry?;
                    if let Some(event) = convert_input(record) {
                        sender.send(event)?;
                    }
                }
                drop(sender);
                let output = engine.process_txs().await;
                print_output(convert_output(output));
            }
            Err(e) => {
                eprintln!("Couldn't create reader: {:?}", e);
            }
        }
    } else {
        eprintln!("Missing path to csv file");
    }
    Ok(())
}
fn convert_input(entry: Input) -> Option<Event> {
    match entry.tx_type.as_str() {
        "deposit" => {
            if entry.amount.is_none() {
                return None;
            }
            return Some(Event::Deposit(Transaction {
                id: entry.tx,
                client: entry.client,
                amount: entry.amount.unwrap(),
                is_locked: false,
            }));
        }
        "withdrawal" => {
            if entry.amount.is_none() {
                return None;
            }
            return Some(Event::Withdrawal(Transaction {
                id: entry.tx,
                client: entry.client,
                amount: entry.amount.unwrap(),
                is_locked: false,
            }));
        }
        "dispute" => {
            if entry.amount.is_some() {
                return None;
            }
            return Some(Event::Dispute {
                client: entry.client,
                tx_id: entry.tx,
            });
        }
        "resolve" => {
            if entry.amount.is_some() {
                return None;
            }
            return Some(Event::Resolve {
                client: entry.client,
                tx_id: entry.tx,
            });
        }
        "chargeback" => {
            if entry.amount.is_some() {
                return None;
            }
            return Some(Event::Chargeback {
                client: entry.client,
                tx_id: entry.tx,
            });
        }
        _ => None,
    }
}
fn convert_output(result: BTreeMap<ClientId, AccountingResult>) -> Vec<Output> {
    let mut output = vec![];
    for (client, entry) in result {
        output.push(Output {
            client: client.to_string(),
            available: entry.available.normalize(),
            held: entry.held.normalize(),
            total: entry.total.normalize(),
            locked: entry.locked
        })
    }
    output
}
fn print_output(output: Vec<Output>) {
    let mut wtr = csv::Writer::from_writer(std::io::stdout());
    for entry in output {
        wtr.serialize(entry).unwrap();
    }
    wtr.flush().unwrap();
}
#[derive(Debug, serde::Deserialize)]
pub struct Input {
    #[serde(rename = "type")]
    tx_type: String,
    client: ClientId,
    tx: TxId,
    amount: Option<Amount>,
}
#[derive(Debug, serde::Serialize)]
pub struct Output {
    client: String,
    available: Amount,
    held: Amount,
    total: Amount,
    locked: bool,
}

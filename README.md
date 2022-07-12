# accounting-engine
A prototype for an accounting engine that processes transactions
and handles disputes and chargebacks. 

## Build from source
```commandline
cargo build
```

## Usage
```commandline
cargo run -- transactions.csv
```
The binary takes a csv file as a parameter 
and outputs the final account balances in csv format
onto the standard output. 

The csv file has the following columns:
- `type` Transaction Type (String): 
deposit, withdrawal, dispute, resolve, chargeback. 
Only *deposit* and *withdrawal* specify their own tx id and amount. 
Every other type specifies the tx id they refer to and no amount
- `client` Client Id (u16): A globally unique identifier for the client account
- `tx` Transaction Id (u32): A globally unique identifier for the transaction
- `amount` Transaction Amount (decimal)

Transactions that can be parsed but are invalid 
will be ignored by the engine.

The produced account balances have the following columns:
- `client` Client Id (u16)
- `available` Available Funds (decimal)
- `held` Funds in Dispute (decimal)
- `total`=`available`+`held` (decimal)
- `locked` If the Account is frozen which happens after a chargeback (bool)

## Testing
```commandline
cargo test
```
Tests the core accounting engine.
#![allow(unused)]
use bitcoin::hex::DisplayHex;
use bitcoincore_rpc::bitcoin::{Address, Amount, Network};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;

// Node access params
const RPC_URL: &str = "http://127.0.0.1:18443"; // Default regtest RPC port
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";

// You can use calls not provided in RPC lib API using the generic `call` function.
// An example of using the `send` RPC call, which doesn't have exposed API.
// You can also use serde_json `Deserialize` derivation to capture the returned json result.
fn send(rpc: &Client, addr: &str) -> bitcoincore_rpc::Result<String> {
    let args = [
        json!([{addr : 100 }]), // recipient address
        json!(null),            // conf target
        json!(null),            // estimate mode
        json!(null),            // fee rate in sats/vb
        json!(null),            // Empty option object
    ];

    #[derive(Deserialize)]
    struct SendResult {
        complete: bool,
        txid: String,
    }
    let send_result = rpc.call::<SendResult>("send", &args)?;
    assert!(send_result.complete);
    Ok(send_result.txid)
}

fn main() -> bitcoincore_rpc::Result<()> {
    // Connect to Bitcoin Core RPC
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Get blockchain info
    let blockchain_info = rpc.get_blockchain_info()?;
    println!("Blockchain Info: {blockchain_info:?}");

    // Create/Load the wallets, named 'Miner' and 'Trader'. Have logic to optionally create/load them if they do not exist or not loaded already.
    let _ = rpc.create_wallet("Miner", Some(false), Some(true), None, None);
    let _ = rpc.create_wallet("Trader", Some(false), Some(true), None, None);

    let miner_rpc = Client::new(
        "http://127.0.0.1:18443/wallet/Miner",
        Auth::UserPass(RPC_USER.to_string(), RPC_PASS.to_string()),
    )?;
    let trader_rpc = Client::new(
        "http://127.0.0.1:18443/wallet/Trader",
        Auth::UserPass(RPC_USER.to_string(), RPC_PASS.to_string()),
    )?;
    // Generate spendable balances in the Miner wallet. How many blocks needs to be mined?
    let miner_addr = miner_rpc
        .get_new_address("Mining Reward".into(), None)?
        .require_network(bitcoincore_rpc::bitcoin::Network::Regtest)
        .map_err(std::io::Error::other)?;

    let blocks = rpc.generate_to_address(103, &miner_addr)?;
    // NOTE: Coinbase rewards take 100 blocks to mature
    // Therefore, mining must continue until at least 100 blocks are generated before the reward is spendable.
    // Mine 103 blocks to make balance spendable
    let miner_balance = miner_rpc.get_balance(None, None)?;
    println!("Miner balance: {} BTC", miner_balance.to_btc());

    // Load Trader wallet and generate a new address
    let trader_addr = trader_rpc
        .get_new_address("Received".into(), None)?
        .require_network(Network::Regtest)
        .unwrap();

    // Send 20 BTC from Miner to Trader
    let txid = miner_rpc.send_to_address(
        &trader_addr,
        Amount::from_btc(20.0).unwrap(),
        None,
        None,
        None,
        None,
        None,
        None,
    )?;
    println!("Sent TXID: {txid}");

    // Check transaction in mempool
    let mempool_tx = rpc.get_mempool_entry(&txid)?;
    println!("Unconfirmed TX: {mempool_tx:?}");

    // Mine 1 block to confirm the transaction
    let _ = rpc.generate_to_address(1, &miner_addr)?;

    // Extract all required transaction details
    let raw_tx = rpc.get_raw_transaction_info(&txid, None)?;
    let decoded_tx = raw_tx.transaction().unwrap();
    let blockhash = raw_tx.blockhash.unwrap();
    let block = rpc.get_block_info(&blockhash)?;
    let block_height = block.height;

    let input = &decoded_tx.input[0];
    let prev_tx = rpc.get_raw_transaction_info(&input.previous_output.txid, None)?;
    let prev_tx_out = &prev_tx.transaction().unwrap().output[input.previous_output.vout as usize];
    let input_amount = prev_tx_out.value;
    let input_address = Address::from_script(&prev_tx_out.script_pubkey, Network::Regtest).unwrap();

    let mut trader_output = None;
    let mut change_output = None;
    for output in &decoded_tx.output {
        let addr = Address::from_script(&output.script_pubkey, Network::Regtest).unwrap();
        if addr == trader_addr {
            trader_output = Some((addr.to_string(), output.value));
        } else {
            change_output = Some((addr.to_string(), output.value));
        }
    }
    let fee = input_amount - trader_output.as_ref().unwrap().1 - change_output.as_ref().unwrap().1;

    // Write the data to ../out.txt in the specified format given in readme.md

    Ok(())
}

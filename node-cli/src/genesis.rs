use byteorder::{ByteOrder, BE};
use keys::Address;
use primitives::H256;
use prost::Message;
use prost_types::Any;
use proto2::chain::{
    block_header::Raw as BlockHeaderRaw, transaction::Contract, transaction::Raw as TransactionRaw, Block, BlockHeader,
    ContractType, Transaction,
};
use proto2::contract::TransferContract;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::mem;

use crate::merkle_tree::MerkleTree;

#[derive(Serialize, Deserialize, Debug)]
struct Witness {
    address: String,
    url: String,
    votes: i64,
}

#[derive(Serialize, Deserialize, Debug)]
struct Alloc {
    address: String,
    name: String,
    balance: i64,
}

impl Alloc {
    fn to_transaction(&self, sender: &[u8]) -> Result<Transaction, Box<dyn Error>> {
        let transfer_contract = TransferContract {
            owner_address: sender.to_owned(),
            to_address: self.address.parse::<Address>()?.as_bytes().to_owned(),
            amount: self.balance,
        };
        let any = Any {
            type_url: "type.googleapis.com/protocol.TransferContract".into(),
            value: {
                let mut buf: Vec<u8> = Vec::with_capacity(255);
                transfer_contract.encode(&mut buf)?;
                buf
            },
        };
        let contract = Contract {
            r#type: ContractType::TransferContract as i32,
            parameter: Some(any).into(),
            ..Default::default()
        };
        let raw = TransactionRaw {
            contract: Some(contract),
            ..Default::default()
        };
        let transaction = Transaction {
            raw_data: Some(raw).into(),
            ..Default::default()
        };
        Ok(transaction)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    timestamp: i64,
    #[serde(rename = "parentHash")]
    parent_hash: String,
    witnesses: Vec<Witness>,
    allocs: Vec<Alloc>,
    mantra: String,
    creator: String,
}

impl GenesisConfig {
    pub fn to_block(&self) -> Result<Block, Box<dyn Error>> {
        let sender = keys::b58decode_check(&self.creator)?;
        let transactions = self
            .allocs
            .iter()
            .map(|alloc| alloc.to_transaction(&sender))
            .collect::<Result<Vec<Transaction>, Box<dyn Error>>>()?;

        let hashes = transactions
            .iter()
            .map(|trx| get_transaction_hash(trx))
            .collect::<Vec<_>>();
        let tree = MerkleTree::from_vec(hashes);

        // mainnet: "8ef446bf3f395af929c218014f6101ec86576c5f61b2ae3236bf3a2ab5e2fecd"
        // nile:    "6556a96828248d6b89cfd0487d4cef82b134b5544dc428c8a218beb2db85ab24"
        // shasta:  "ea97ca7ac977cf2765093fa0e4732e561dc4ff8871c17e35fd2bcabb8b5f821d"
        println!("txTrieRoot => {:?}", tree.root_hash());

        let raw_header = BlockHeaderRaw {
            number: 0,
            timestamp: self.timestamp,
            witness_address: self.mantra.as_bytes().to_owned(),
            parent_hash: parse_hex(&self.parent_hash),
            merkle_root_hash: tree.root_hash().as_bytes().to_owned(),
            ..Default::default()
        };
        let header = BlockHeader {
            raw_data: Some(raw_header).into(),
            ..Default::default()
        };
        let block = Block {
            block_header: Some(header).into(),
            transactions: transactions.into(),
            ..Default::default()
        };

        Ok(block)
    }
}

fn parse_hex(encoded: &str) -> Vec<u8> {
    if encoded.starts_with("0x") || encoded.starts_with("0X") {
        hex::decode(&encoded[2..]).unwrap()
    } else {
        hex::decode(encoded).unwrap()
    }
}

pub fn get_transaction_hash(transaction: &Transaction) -> H256 {
    let mut sha256 = Sha256::new();
    let mut buf: Vec<u8> = Vec::with_capacity(255);
    transaction.encode(&mut buf).unwrap();
    sha256.input(&buf);
    unsafe { mem::transmute(sha256.result()) }
}

pub fn calculate_block_id(block: &Block) -> H256 {
    let mut sha256 = Sha256::new();
    let mut buf: Vec<u8> = Vec::with_capacity(255);
    let raw_header = &block.block_header.as_ref().unwrap().raw_data.as_ref().unwrap();
    let block_numer = raw_header.number;
    raw_header.encode(&mut buf).unwrap();
    sha256.input(&buf);
    let mut block_hash: H256 = unsafe { mem::transmute(sha256.result()) };
    BE::write_i64(&mut block_hash[..8], block_numer);
    block_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    #[ignore]
    fn load_genesis_json() {
        let content = fs::read_to_string("./genesis.json").unwrap();
        let conf: GenesisConfig = serde_json::from_str(&content).unwrap();
        // println!("got =>\n{:?}", conf);
        let block = conf.to_block().unwrap();
        println!("block => {:?}", block);
        println!("block_id => {:?}", calculate_block_id(&block));
    }
}
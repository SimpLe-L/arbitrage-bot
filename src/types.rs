use std::fmt;
use ethers::types::{TransactionRequest, TransactionReceipt, Log, H256};
use serde::{Deserialize, Serialize};
use crate::engine::executor::telegram_message::Message;

#[derive(Debug, Clone)]
pub enum Action {
    NotifyViaTelegram(Message), 
    ExecutePublicTx(TransactionRequest),
    MevRelaySubmitBid((TransactionRequest, u64, H256)),
}

impl From<Message> for Action {
    fn from(msg: Message) -> Self {
        Self::NotifyViaTelegram(msg)
    }
}

impl From<TransactionRequest> for Action {
    fn from(tx: TransactionRequest) -> Self {
        Self::ExecutePublicTx(tx)
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    PublicTx(TransactionReceipt, Vec<Log>),
    PendingTx(ethers::types::Transaction),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Source {
    Public,
    Mempool,
    MevRelay {
        opp_tx_hash: H256,
        bid_amount: u64,
        start: u64,
        deadline: u64,
        arb_found: u64,
    },
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Public => write!(f, "Public"),
            Source::Mempool => write!(f, "Mempool"),
            Source::MevRelay { .. } => write!(f, "MevRelay"),
        }
    }
}

impl Source {
    pub fn is_mempool(&self) -> bool {
        matches!(self, Source::Mempool)
    }

    pub fn is_mev_relay(&self) -> bool {
        matches!(self, Source::MevRelay { .. })
    }

    pub fn deadline(&self) -> Option<u64> {
        match self {
            Source::MevRelay { deadline, .. } => Some(*deadline),
            _ => None,
        }
    }

    pub fn with_arb_found_time(self, arb_found: u64) -> Self {
        match self {
            Source::MevRelay { 
                opp_tx_hash,
                bid_amount,
                start,
                deadline,
                arb_found: _,
            } => Source::MevRelay {
                opp_tx_hash,
                bid_amount,
                start,
                deadline,
                arb_found,
            },
            _ => self,
        }
    }

    pub fn with_bid_amount(self, bid_amount: u64) -> Self {
        match self {
            Source::MevRelay { 
                opp_tx_hash,
                bid_amount: _,
                start,
                deadline,
                arb_found,
            } => Source::MevRelay {
                opp_tx_hash,
                bid_amount,
                start,
                deadline,
                arb_found,
            },
            _ => self,
        }
    }
}

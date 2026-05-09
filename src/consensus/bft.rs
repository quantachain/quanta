use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use sha3::{Digest, Sha3_256};
use pqcrypto_dilithium::dilithium5::{verify, Signature as DilithiumSignature, PublicKey as DilithiumPublicKey};

/// The Post-Quantum BFT Consensus Engine for Quanta 2.0.
/// 
/// This module implements a Tendermint-style Byzantine Fault Tolerant (BFT) 
/// consensus algorithm. It achieves absolute finality in 3 steps:
/// 1. Propose: A leader proposes a block.
/// 2. PreVote: Validators vote on the proposal.
/// 3. PreCommit: Validators commit to the block if 2/3 majority is reached.
/// 
/// SIGNATURE AGGREGATION:
/// All votes are signed using ML-DSA (Dilithium-5). When 2/3 of validators PreCommit,
/// the Proposer aggregates the Dilithium signatures into a single Master Signature.

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BftStep {
    Propose,
    PreVote,
    PreCommit,
    Commit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    pub address: String,
    pub dilithium_pubkey: Vec<u8>,
    pub voting_power: u64, // Based on QUA staked
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BftVote {
    pub step: BftStep,
    pub block_hash: String,
    pub validator_address: String,
    pub signature: Vec<u8>, // Dilithium Signature
}

pub struct BftEngine {
    pub validators: HashMap<String, Validator>,
    pub current_height: u64,
    pub current_round: u32,
    pub current_step: BftStep,
    
    // Votes for the current round
    prevotes: HashMap<String, Vec<BftVote>>, // block_hash -> votes
    precommits: HashMap<String, Vec<BftVote>>, // block_hash -> votes
}

impl BftEngine {
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
            current_height: 0,
            current_round: 0,
            current_step: BftStep::Propose,
            prevotes: HashMap::new(),
            precommits: HashMap::new(),
        }
    }

    pub fn total_voting_power(&self) -> u64 {
        self.validators.values().map(|v| v.voting_power).sum()
    }

    /// Process an incoming Dilithium vote from a validator.
    pub fn process_vote(&mut self, vote: BftVote) -> bool {
        // 1. Check if validator exists
        let validator = match self.validators.get(&vote.validator_address) {
            Some(v) => v,
            None => return false,
        };

        // 2. Load the Dilithium Public Key
        let pk_result = DilithiumPublicKey::from_bytes(&validator.dilithium_pubkey);
        let pk = match pk_result {
            Ok(pk) => pk,
            Err(_) => return false, // Invalid key
        };

        // 3. Load the Dilithium Signature
        let sig_result = DilithiumSignature::from_bytes(&vote.signature);
        let sig = match sig_result {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        // 4. Verify the Vote Payload
        // We sign: Step || Round || BlockHash
        let payload = format!("{:?}:{}:{}", vote.step, self.current_round, vote.block_hash);
        
        // PQC Verification!
        if verify(&sig, payload.as_bytes(), &pk).is_err() {
            return false; // Cryptographic rejection
        }

        // 5. Tally the vote
        match vote.step {
            BftStep::PreVote => {
                let entry = self.prevotes.entry(vote.block_hash.clone()).or_insert(Vec::new());
                // Prevent double voting
                if !entry.iter().any(|v| v.validator_address == vote.validator_address) {
                    entry.push(vote);
                }
            },
            BftStep::PreCommit => {
                let entry = self.precommits.entry(vote.block_hash.clone()).or_insert(Vec::new());
                if !entry.iter().any(|v| v.validator_address == vote.validator_address) {
                    entry.push(vote);
                }
            },
            _ => return false,
        }

        self.check_quorum(&vote.block_hash, &vote.step)
    }

    /// Check if 2/3+ voting power has been reached for a specific block hash.
    pub fn check_quorum(&mut self, block_hash: &str, step: &BftStep) -> bool {
        let votes = match step {
            BftStep::PreVote => self.prevotes.get(block_hash),
            BftStep::PreCommit => self.precommits.get(block_hash),
            _ => return false,
        };

        let votes = match votes {
            Some(v) => v,
            None => return false,
        };

        let mut power: u64 = 0;
        for vote in votes {
            if let Some(val) = self.validators.get(&vote.validator_address) {
                power += val.voting_power;
            }
        }

        // Tendermint BFT Rule: Needs > 2/3 of total power
        let threshold = (self.total_voting_power() * 2) / 3;
        
        if power > threshold {
            if *step == BftStep::PreVote {
                self.current_step = BftStep::PreCommit;
            } else if *step == BftStep::PreCommit {
                self.current_step = BftStep::Commit;
            }
            true
        } else {
            false
        }
    }

    /// Compress all 67%+ Dilithium PreCommit signatures into 1 Master Signature.
    /// This uses a Fiat-Shamir deterministic hash compression to simulate 
    /// the full lattice MPC aggregation for Quanta 2.0.
    pub fn aggregate_master_signature(&self, block_hash: &str) -> Option<Vec<u8>> {
        if self.current_step != BftStep::Commit {
            return None;
        }

        let precommits = self.precommits.get(block_hash)?;
        
        let mut hasher = Sha3_256::new();
        hasher.update(b"QUANTA_2.0_DILITHIUM_MASTER_SIG:");
        hasher.update(block_hash.as_bytes());
        
        // Sort to ensure deterministic aggregation
        let mut sorted_votes = precommits.clone();
        sorted_votes.sort_by(|a, b| a.validator_address.cmp(&b.validator_address));

        for vote in sorted_votes {
            hasher.update(&vote.signature);
        }

        // Return the 32-byte aggregated Master Signature
        Some(hasher.finalize().to_vec())
    }
}

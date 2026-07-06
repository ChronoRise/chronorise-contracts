#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Map, String, Vec,
};

// ─── Types ────────────────────────────────────────────────────────────────────

/// Status of a governance proposal.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum ProposalStatus {
    Active,
    Passed,
    Rejected,
    Executed,
    Cancelled,
}

/// A governance proposal.
#[contracttype]
#[derive(Clone)]
pub struct Proposal {
    pub title: String,
    pub description: String,
    pub proposer: Address,
    pub start_ledger: u32,
    /// Voting period ends at this ledger (exclusive)
    pub end_ledger: u32,
    pub votes_for: i128,
    pub votes_against: i128,
    pub status: ProposalStatus,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    /// Governance token contract address (instance)
    GovToken,
    /// Minimum combined vote weight for a proposal to be valid (instance)
    Quorum,
    /// Minimum yes-vote share to pass, in basis points (instance)
    ApprovalBps,
    /// Voting period in ledgers (instance)
    VotingPeriod,
    /// Next proposal ID counter (instance)
    NextProposalId,
    /// Proposal keyed by ID (persistent)
    Proposal(u32),
    /// Map<Address, i128>: actual token weight used per voter per proposal (persistent)
    Voted(u32),
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct GovernanceContract;

#[contractimpl]
impl GovernanceContract {
    // ── Init ──────────────────────────────────────────────────────────────────

    /// Initialise governance.
    /// Parameters: admin, gov_token, quorum, approval_bps, voting_period.
    pub fn initialize(
        env: Env,
        admin: Address,
        gov_token: Address,
        quorum: i128,
        approval_bps: u32,
        voting_period: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        assert!(approval_bps <= 10_000, "approval_bps exceeds 10000");
        assert!(quorum > 0, "quorum must be positive");
        assert!(voting_period > 0, "voting_period must be positive");
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::GovToken, &gov_token);
        env.storage().instance().set(&DataKey::Quorum, &quorum);
        env.storage()
            .instance()
            .set(&DataKey::ApprovalBps, &approval_bps);
        env.storage()
            .instance()
            .set(&DataKey::VotingPeriod, &voting_period);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &0_u32);
    }

    // ── Admin helpers ─────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Address {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        admin
    }

    // ── Config update ─────────────────────────────────────────────────────────

    /// Admin: update governance parameters.
    pub fn update_config(
        env: Env,
        quorum: i128,
        approval_bps: u32,
        voting_period: u32,
    ) {
        Self::require_admin(&env);
        assert!(approval_bps <= 10_000, "approval_bps exceeds 10000");
        assert!(quorum > 0, "quorum must be positive");
        assert!(voting_period > 0, "voting_period must be positive");

        env.storage().instance().set(&DataKey::Quorum, &quorum);
        env.storage()
            .instance()
            .set(&DataKey::ApprovalBps, &approval_bps);
        env.storage()
            .instance()
            .set(&DataKey::VotingPeriod, &voting_period);
    }

    // ── Proposals ─────────────────────────────────────────────────────────────

    /// Create a proposal. Returns the new proposal ID.
    pub fn propose(
        env: Env,
        proposer: Address,
        title: String,
        description: String,
    ) -> u32 {
        proposer.require_auth();

        let voting_period: u32 = env
            .storage()
            .instance()
            .get(&DataKey::VotingPeriod)
            .expect("not initialized");

        let start = env.ledger().sequence();
        let end = start + voting_period;

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap_or(0);

        let proposal = Proposal {
            title,
            description,
            proposer,
            start_ledger: start,
            end_ledger: end,
            votes_for: 0,
            votes_against: 0,
            status: ProposalStatus::Active,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(id), &proposal);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &(id + 1));
        env.storage()
            .persistent()
            .set(&DataKey::Voted(id), &Map::<Address, i128>::new(&env));

        env.events()
            .publish((symbol_short!("propose"),), (id, env.ledger().sequence()));
        id
    }

    // ── Voting ────────────────────────────────────────────────────────────────

    /// Cast a vote on a proposal.
    ///
    /// The vote weight is the voter's **actual on-chain governance token
    /// balance** at the time of voting. The caller cannot self-report their
    /// weight — it is read directly from the governance token contract.
    ///
    /// Each address may vote exactly once per proposal.
    pub fn vote(env: Env, voter: Address, proposal_id: u32, support: bool) {
        voter.require_auth();

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");

        assert!(proposal.status == ProposalStatus::Active, "proposal not active");
        assert!(env.ledger().sequence() <= proposal.end_ledger, "voting period ended");

        let mut voted: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&DataKey::Voted(proposal_id))
            .unwrap_or_else(|| Map::new(&env));
        assert!(voted.get(voter.clone()).is_none(), "already voted");

        // Read the voter's actual balance from the governance token contract.
        let gov_token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::GovToken)
            .expect("not initialized");
        let token_client = token::Client::new(&env, &gov_token_id);
        let weight = token_client.balance(&voter);

        assert!(weight > 0, "zero token balance — cannot vote");

        voted.set(voter.clone(), weight);
        env.storage()
            .persistent()
            .set(&DataKey::Voted(proposal_id), &voted);

        if support {
            proposal.votes_for += weight;
        } else {
            proposal.votes_against += weight;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((symbol_short!("vote"),), (proposal_id, voter, weight, support));
    }

    // ── Finalisation ─────────────────────────────────────────────────────────

    /// Anyone may call this once the voting period has ended.
    pub fn finalise(env: Env, proposal_id: u32) {
        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");

        assert!(proposal.status == ProposalStatus::Active, "proposal not active");
        assert!(env.ledger().sequence() > proposal.end_ledger, "voting still in progress");

        let quorum: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Quorum)
            .expect("not initialized");
        let approval_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ApprovalBps)
            .expect("not initialized");

        let total = proposal.votes_for + proposal.votes_against;
        let passed = total >= quorum
            && proposal.votes_for * 10_000 >= total * approval_bps as i128;

        proposal.status = if passed {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Rejected
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((symbol_short!("finalise"),), (proposal_id, passed));
    }

    // ── Execution ─────────────────────────────────────────────────────────────

    /// Admin: mark a Passed proposal as Executed.
    pub fn mark_executed(env: Env, proposal_id: u32) {
        Self::require_admin(&env);

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");
        assert!(proposal.status == ProposalStatus::Passed, "proposal not passed");

        proposal.status = ProposalStatus::Executed;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((symbol_short!("executed"),), proposal_id);
    }

    /// Admin: cancel an Active proposal.
    pub fn cancel(env: Env, proposal_id: u32) {
        Self::require_admin(&env);

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");
        assert!(proposal.status == ProposalStatus::Active, "proposal not active");

        proposal.status = ProposalStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    pub fn get_proposal(env: Env, proposal_id: u32) -> Proposal {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found")
    }

    pub fn has_voted(env: Env, proposal_id: u32, voter: Address) -> bool {
        let voted: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&DataKey::Voted(proposal_id))
            .unwrap_or_else(|| Map::new(&env));
        voted.get(voter).is_some()
    }

    /// Return the weight used by `voter` in `proposal_id` (0 if not voted).
    pub fn vote_weight(env: Env, proposal_id: u32, voter: Address) -> i128 {
        let voted: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&DataKey::Voted(proposal_id))
            .unwrap_or_else(|| Map::new(&env));
        voted.get(voter).unwrap_or(0)
    }

    pub fn proposal_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap_or(0)
    }

    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn gov_token(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::GovToken)
            .expect("not initialized")
    }

    pub fn config(env: Env) -> (i128, u32, u32) {
        let quorum: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Quorum)
            .unwrap_or(0);
        let approval_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ApprovalBps)
            .unwrap_or(0);
        let voting_period: u32 = env
            .storage()
            .instance()
            .get(&DataKey::VotingPeriod)
            .unwrap_or(0);
        (quorum, approval_bps, voting_period)
    }
}

mod test;

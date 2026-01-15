# Governance Architecture Design

Full architecture for Community Governance features, incorporating the 3-phase voting process and delegation system.

## Overview

A hybrid on-ledger/off-ledger system for Radix community governance:

- **On-ledger (trustless):** Proposals, votes, delegation registry
- **Off-chain (verifiable):** Vote counting, LSU→XRD conversion, result storage
- **External (data source):** Radix Gateway API for historical balances

## Governance Process

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│ Phase 1     │     │ Phase 2          │     │ Phase 3     │
│ RFC         │────►│ Temperature Check│────►│ RFP         │
│ (off-chain) │     │ (on-chain vote)  │     │ (on-chain)  │
└─────────────┘     └──────────────────┘     └─────────────┘
  RadixTalk           For/Against vote        Full proposal
  discussion          on merit                vote with options
```

1. **RFC (Request for Comment):** Draft proposal posted on RadixTalk for community discussion. Not on-chain.
2. **Temperature Check:** Proposal pushed on-chain. Community votes For/Against elevating to RFP. Requires quorum.
3. **RFP (Request for Proposal):** Passed temperature checks are elevated (by OWNER). Full vote on proposal options.

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                         FRONTEND (React)                         │
│  Proposals UI │ Voting UI │ Delegation UI │ Verification UI      │
└───────────────────────────┬──────────────────────────────────────┘
                            │
         ┌──────────────────┼──────────────────┐
         ▼                  │                  ▼
┌─────────────────┐         │         ┌─────────────────┐
│ BACKEND (Effect)│         │         │  RADIX WALLET   │
│ ─────────────── │         │         │  (sign txns)    │
│ • ProposalSvc   │         │         └────────┬────────┘
│ • DelegationSvc │         │                  │
│ • VoteCountSvc  │         │                  ▼
│ • API routes    │         │         ┌─────────────────────────┐
└────────┬────────┘         │         │     ON-LEDGER           │
         │                  │         │ ┌─────────────────────┐ │
         │                  │         │ │ Governance Component│ │
         ▼                  │         │ │ • temp_checks KVS   │ │
┌─────────────────┐         │         │ │ • proposals KVS     │ │
│   POSTGRES      │         │         │ │ • votes KVS         │ │
│ (results cache) │         │         │ └─────────────────────┘ │
└─────────────────┘         │         │ ┌─────────────────────┐ │
         ▲                  │         │ │VoteDelegation Comp. │ │
         │                  │         │ │ • delegatees KVS    │ │
┌────────┴────────┐         │         │ │ • delegators KVS    │ │
│  GATEWAY API    │◄────────┘         │ └─────────────────────┘ │
│ (LSU balances)  │                   └─────────────────────────┘
└─────────────────┘
```

### Trust Model

| Layer | Trust Level | Verification |
|-------|-------------|--------------|
| On-ledger (contracts) | Trustless | Anyone can read/verify state |
| Off-chain (backend) | Verifiable | Deterministic calculation from public inputs |
| External (Gateway) | Trusted | Radix's official API |

## On-Ledger Components

Two separate Scrypto components for upgradeability — governance logic can change without users losing delegation setup.

### Governance Component

Stores temperature checks and proposals.

```rust
pub struct Governance {
    pub governance_parameters: GovernanceParameters,
    pub temperature_checks: KeyValueStore<u64, TemperatureCheck>,
    pub temperature_check_count: u64,
    pub proposals: KeyValueStore<u64, Proposal>,
    pub proposal_count: u64,
}

pub struct GovernanceParameters {
    pub temperature_check_days: u16,
    pub temperature_check_quorum: Decimal,
    pub temperature_check_approval_threshold: Decimal,
    pub temperature_check_propose_threshold: Decimal,
    pub proposal_length_days: u16,
    pub proposal_quorum: Decimal,
    pub proposal_approval_threshold: Decimal,
}

pub struct TemperatureCheck {
    pub title: String,
    pub description: String,
    pub vote_options: Vec<ProposalVoteOption>,
    pub attachments: Vec<File>,
    pub rfc_url: Url,
    pub quorum: Decimal,
    pub votes: KeyValueStore<Global<Account>, TemperatureCheckVote>,
    pub approval_threshold: Decimal,
    pub start: Instant,
    pub deadline: Instant,
    pub elevated_proposal_id: Option<u64>,
}

pub struct Proposal {
    pub title: String,
    pub description: String,
    pub vote_options: Vec<ProposalVoteOption>,
    pub attachments: Vec<File>,
    pub rfc_url: Url,
    pub quorum: Decimal,
    pub votes: KeyValueStore<Global<Account>, ProposalVoteOptionId>,
    pub approval_threshold: Decimal,
    pub start: Instant,
    pub deadline: Instant,
    pub temperature_check_id: u64,
}
```

#### API

| Method | Auth | Description |
|--------|------|-------------|
| `instantiate()` | PUBLIC | Create governance component with owner badge |
| `make_temperature_check()` | PUBLIC | Create temp check (requires XRD threshold) |
| `make_proposal()` | OWNER | Elevate passed temp check to RFP |
| `vote_on_temperature_check()` | PUBLIC | Vote For/Against elevation |
| `vote_on_proposal()` | PUBLIC | Vote on RFP options |

### VoteDelegation Component

Stores delegation relationships separately for upgradeability.

```rust
pub struct VoteDelegation {
    // Who has delegated TO this account
    pub delegatees: KeyValueStore<Global<Account>, KeyValueStore<Global<Account>, Decimal>>,
    // This account's delegations to others
    pub delegators: KeyValueStore<Global<Account>, Vec<Delegation>>,
}

pub struct Delegation {
    pub delegatee: Global<Account>,
    pub fraction: Decimal,
    pub valid_until: Instant,
}
```

#### API

| Method | Auth | Description |
|--------|------|-------------|
| `make_delegation()` | PUBLIC | Delegate voting power (caller must be present) |
| `remove_delegation()` | PUBLIC | Remove delegation to specific account |

## Off-Chain Backend (Effect)

### Service Architecture

```
┌─────────────────────────────────────────────┐
│              HttpApi (routes)               │
├─────────────────────────────────────────────┤
│  ProposalService   │   DelegationService    │
├────────────────────┼────────────────────────┤
│         VoteCountingService                 │
├─────────────────────────────────────────────┤
│  GatewayClient  │  LedgerClient  │  Database │
└─────────────────────────────────────────────┘
```

### Tech Stack

| Layer | Tech | Rationale |
|-------|------|-----------|
| API | Effect + @effect/platform | Type-safe errors, composable services |
| Database | Postgres via @effect/sql | Effect-native SQL |
| Queue | Effect Schedule + Queue | Built-in retry/backoff |
| Gateway | Wrapped in Effect Service | Testable, typed errors |

### Vote Counting Pipeline

```
1. COLLECT VOTES
   Query Governance component → votes KVS
   Result: Map<Account, Choice>

2. RESOLVE DELEGATION
   For each voter:
   ├─ Query delegatees KVS → who delegated TO them?
   └─ Query delegators KVS → did they delegate AWAY power?
   Result: Map<Account, voting_power_fraction>

3. FETCH BALANCES
   Query Gateway API at snapshot state version
   └─ Get LSU holdings → convert to XRD
   Result: Map<Account, XRD_balance>

4. CALCULATE RESULTS
   For each vote:
     weight = XRD_balance × voting_power_fraction
   Aggregate by choice
   Check quorum + approval_threshold
   Result: { totals, quorum_met, passed }
```

### API Endpoints

```
GET  /proposals                    # List all (temp checks + RFPs)
GET  /proposals/:id                # Single proposal with results
GET  /proposals/:id/votes          # Vote breakdown
GET  /delegation/:account          # Account's delegation config
POST /admin/recalculate/:id        # Trigger recalculation
```

## Frontend

### Data Sources by Feature

| Feature | Source | Rationale |
|---------|--------|-----------|
| List proposals | Backend API | Faster, includes results |
| Proposal details | Backend API | Includes vote totals |
| Cast vote | Contract (wallet) | Trustless |
| Set delegation | Contract (wallet) | Trustless |
| View my votes | Contract (read) | Verify recording |
| View my delegation | Contract (read) | Verify setup |

### User Flows

**Voting:**
1. View proposal (backend)
2. Select choice, click "Vote"
3. Wallet prompts for signature
4. Transaction submitted
5. Frontend polls until results update

**Delegation:**
1. Enter delegatee + percentage + expiry
2. Wallet prompts for signature
3. Transaction submitted
4. Confirm delegation recorded

## Error Handling

### On-Chain (Contract Enforced)

| Scenario | Handling |
|----------|----------|
| Vote after deadline | Contract rejects |
| Vote on non-existent proposal | Contract rejects |
| Delegation exceeds 100% | Contract rejects |
| Double vote | Contract rejects (no vote changing) |

### Off-Chain (Effect Error Types)

| Scenario | Error Type | Handling |
|----------|------------|----------|
| Gateway timeout | GatewayError | Retry with backoff |
| Partial data | IncompleteDataError | Mark "calculating", retry later |
| LSU conversion fails | LsuConversionError | Log, skip, flag for review |
| Circular delegation | DelegationCycleError | Exclude cycle participants |

## Testing Strategy

### Contract (Scrypto)

- **Unit tests:** Valid/invalid inputs for each method
- **Integration tests:** Full flow on local simulator

### Backend (Effect)

- **Services:** Unit test with mock layers
- **Vote counting:** Property-based tests (fast-check)
- **API routes:** Integration tests with test database

### Verification (Critical)

- **Golden file tests:** Fixed inputs → expected JSON output
- **Cross-implementation:** Python script vs Effect backend must match
- **Mainnet shadow:** Compare against real historical proposals

## Open Questions

1. **Delegator votes after delegatee:** Should direct vote override delegation, or be rejected?
2. **Spam protection threshold:** What XRD amount required to create temp check?
3. **Quorum values:** Specific numbers for GovernanceParameters
4. **File storage:** Use radix-file-storage or hash + IPFS?

## References

- [Consultation dApp v2 Proposal](../consultation-dapp-v2-proposal.md)
- [Community Governance Requests](../community-governance-requests.md)
- [Governance Scrypto Blueprints](../meetings/Consultation%20-%20Governance%20Scrypto%20Bluepints-20260115091629.md)
- [Uniswap Governance Reference](../uniswap-governance-reference.md)

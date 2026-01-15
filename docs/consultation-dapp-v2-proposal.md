# Consultation dApp v2 Proposal

## Overview

The Radix Foundation is winding down and handing control to the community. As part of this transition, the Consultation dApp becomes the primary tool for community decision-making — proposals are put to a vote, and the outcome is respected and executed on.

Version 1 has served us well, but relies on centralized infrastructure that won't be viable once the Foundation steps back. We're proposing v2 with a focus on **decentralization** and **simplicity** so it can run independently.

## What's Changing

| Aspect | v1 (Current) | v2 (Proposed) |
|--------|--------------|---------------|
| **Identity** | Persona-based authentication | Account-based (sign transaction to vote) |
| **Vote storage** | Postgres database | On-ledger (contract) |
| **Token weighting** | Time-weighted average (TWA) of XRD over a period | XRD balance at a single snapshot |
| **Results storage** | Centralized Node.js service | Node.js app run by host (open source, verifiable) |


## Why These Changes

**Remove Persona dependency**
Personas add friction and aren't necessary. Signing a vote transaction from an account already proves control.

**Move votes on-ledger**
A centralized database creates a single point of failure and requires ongoing maintenance. On-ledger storage is transparent, verifiable, and doesn't depend on Foundation infrastructure.

**Simplify to single snapshot**
TWA calculations are complex and opaque. A single snapshot (defined when the proposal is created) is easier to understand and verify.


## How It Works

1. **Create a proposal** — Anyone (or permissioned creators) submits a proposal on-ledger, including the proposal text, vote options, and a snapshot date.

2. **Cast a vote** — Users sign a transaction from their account to register their vote (e.g., For / Against / Abstain). The vote is recorded on-ledger.

3. **Calculate results** — After voting ends, the app reads all votes from the ledger, queries the Gateway API for each voter's XRD balance at the snapshot date, calculates the weighted totals, and stores results in a database.

4. **Display results** — The dApp frontend displays results from its database. Anyone can independently verify by running the open-source calculation script against the ledger and Gateway data.


## Architecture

```
┌─────────────────┐      ┌─────────────────┐
│    Frontend     │◄────►│    Contract     │
│    (dApp)       │      │ (proposals,     │
└────────┬────────┘      │  votes)         │
         │               └─────────────────┘
         ▼
┌─────────────────┐      ┌─────────────────┐
│    Backend      │◄────►│   Gateway API   │
│   + Database    │      └─────────────────┘
│   (results)     │
└─────────────────┘
```

The contract stores proposals and votes (trustless, on-ledger). Results are calculated off-ledger by a Node.js app and stored in a database managed by whoever hosts the dApp. Anyone can host the dApp, and since the source data comes from the ledger and Gateway API, all hosts should produce the same results. This is a tradeoff: we avoid the complexity of permissioning who can write results on-ledger, while keeping the system verifiable.


## Open Questions

**Spam protection**
How should we prevent proposal spam? Options include:
- Require locking XRD to create a proposal
- Minimum XRD balance requirement
- Permissioned proposal creation (badge/allowlist)


*This proposal is part of the Foundation's transition effort to leave the community with functional, decentralized tools for decision-making.*

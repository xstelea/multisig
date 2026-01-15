# Uniswap Governance Reference

Source: [Community Governance Process Update [Jan 2023]](https://gov.uniswap.org/t/community-governance-process-update-jan-2023/19976)

Reference for a mature governance process used by Uniswap.

## Phase 1: RFC (Request for Comment)

- **Timeframe:** Minimum 7 days
- **Format:** Forum post titled "RFC - [Title]"
- **Purpose:** Community digests, comments, asks questions
- **Outcome:** Incorporate feedback into next phase

## Phase 2: Temperature Check

- **Timeframe:** 5 days
- **Threshold:** 10k UNI to propose
- **Quorum:** 10M UNI
- **Format:** Snapshot poll (off-chain)
- **Requirements:** Must include "No change" option
- **Outcome:** Needs 10M UNI yes votes to proceed to Phase 3

## Phase 3: Governance Proposal (On-chain)

- **Timeframe:** 2 day waiting + 6 day voting + 2 day timelock
- **Threshold:** 1M UNI to propose
- **Quorum:** 40M UNI
- **Format:** On-chain vote
- **Outcome:** If passed, code is executed automatically

## Process Changes

- 7-day Snapshot vote with 40M UNI quorum required to change governance rules

## Relevance to Consultation dApp

Uniswap's process is more formal and multi-staged than the v2 proposal scope. Key differences:

| Aspect | Uniswap | Consultation dApp v2 |
|--------|---------|---------------------|
| Stages | 3 phases (RFC, Temperature Check, On-chain) | Single vote |
| Off-chain voting | Snapshot | Custom dApp |
| On-chain execution | Automatic via timelock | Manual (Foundation executes) |
| Delegation | Yes | Not in MVP |
| Quorum | Required | TBD |

Features like multi-phase voting, delegation, and quorum could be considered for future iterations.

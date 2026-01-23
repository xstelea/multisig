# Multisig Architecture

Technical design for the synchronous multisig solution that will hold DAO funds after the Foundation transition.

## Context

The Radix Foundation is winding down and funds will move into a community-controlled multisig. This document captures the agreed architecture from the [Jan 14 design session](../meetings/Multisig%20–%202026_01_14%2015_53%20CET%20–%20Notes%20by%20Gemini.md).

## Requirements

- Support the **Radix Wallet** as a signing source
- Allow flexibility for other signers (custodians, private keys)
- Work with DAO representatives as key holders
- Handle complex access rules without hitting fee loan limits

## Design Decisions

### Synchronous Multisig

We chose **synchronous** (off-chain signature collection) over asynchronous (on-chain signature transactions):

| Approach | Pros | Cons |
|----------|------|------|
| Synchronous | Simpler, single transaction submission | Requires orchestrator (DApp) |
| Asynchronous | Fully on-chain, decentralized | Complex state management, multiple transactions |

The legal structure (e.g. Duna corporation or similar) provides alternative enforcement mechanisms, reducing the need for fully trustless on-chain coordination.

### Account-Based (Not Access Controller)

For the initial implementation, we're using a standard **account with multisig access rules** rather than a separate access controller component. An access controller is only needed when different rules are required for primary/recovery/confirmation roles.

### Sub-Intents for Fee Handling

**The Problem:** Complex access rules require more XRD to validate. A multi-signature account with a complex rule will fail *before* reaching `lock_fee` because signature validation happens first and exhausts the initial fee loan.

**The Solution:** Use sub-intents:

```
┌─────────────────────────────────────────────────────────┐
│                    DApp (Orchestrator)                   │
├─────────────────────────────────────────────────────────┤
│  1. Create sub-intent containing DAO action              │
│  2. Collect signatures off-chain from DAO members        │
│  3. Any party can wrap and submit the final transaction  │
│  4. Submitter (single-sig account) pays fees             │
└─────────────────────────────────────────────────────────┘
```

This decouples the complex access rule validation from fee payment. The submitter's simple account pays fees, avoiding the fee loan problem entirely.

### Reverse Mapping

The wallet needs to map NFT Global IDs back to signing keys:

```
Non-Fungible Global ID → (Derivation Path + Factor Source ID)
```

**Initial implementation:** Assume NFT Global IDs were derived from existing user accounts in the wallet. This covers the common case and can be extended later.

### Dedicated Key Space for Multisig

To prevent security issues where an attacker configures an account with a user's public key:

- Use a **separate derivation path** for multisig/MFA keys
- These keys are never used to create accounts
- Prevents cross-contamination between account ownership and signing authority

## Architecture

```
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   Signer 1   │         │   Signer 2   │         │   Signer N   │
│   (Wallet)   │         │   (Wallet)   │         │  (Custodian) │
└──────┬───────┘         └──────┬───────┘         └──────┬───────┘
       │                        │                        │
       │ Sign sub-intent        │ Sign sub-intent        │ Sign sub-intent
       ▼                        ▼                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                         DApp (Orchestrator)                      │
│  - Creates sub-intents for proposals                            │
│  - Collects signatures from authorized signers                   │
│  - Tracks proposal status and expiry                            │
│  - Previews composed intent to verify readiness                  │
└─────────────────────────────────────────────────────────────────┘
       │
       │ Submit (any party can do this)
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Radix Network                            │
│  - DAO Account with multisig access rule                        │
│  - Sub-intent validated against access rule                      │
│  - Submitter pays transaction fees                               │
└─────────────────────────────────────────────────────────────────┘
```

## Sub-Intent Flow

1. **Proposal created** — DApp generates a sub-intent with the proposed action
2. **Author sets expiry** — Expiry is encoded in the manifest (more secure than DApp-controlled)
3. **Signatures collected** — Each signer reviews and signs via their wallet
4. **Readiness check** — DApp previews composed intent to verify sufficient signatures
5. **Submission** — Any party wraps the sub-intent and submits, paying fees from their account
6. **Execution** — Network validates signatures against DAO account's access rule

## Example: DAO Withdrawal with Fee Payer

A concrete example showing how fee locking happens in the main transaction intent (not a subintent):
- **Main transaction intent**: Fee locking + orchestration (signed by submitter who pays fees)
- **DAO treasury withdrawal subintent**: 3 of 4 signers (DAO members)

### Transaction Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        MAIN TRANSACTION INTENT                               │
│                        (Signed by fee payer, notarized by orchestrator)     │
├─────────────────────────────────────────────────────────────────────────────┤
│  USE_CHILD NamedIntent("dao_withdraw") Intent("<subintent_hash>");          │
│                                                                             │
│  CALL_METHOD                                                                │
│    Address("fee_payer_account")                                             │
│    "lock_fee"                                                               │
│    Decimal("100");                                                          │
│                                                                             │
│  YIELD_TO_CHILD NamedIntent("dao_withdraw");                                │
└─────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
                         ┌─────────────────────────────────────────┐
                         │  SUBINTENT: dao_withdraw                 │
                         │  (3 of 4 signers - DAO members)          │
                         ├─────────────────────────────────────────┤
                         │  CALL_METHOD                             │
                         │    Address("dao_treasury")               │
                         │    "withdraw"                            │
                         │    Address("resource_rdx1...")           │
                         │    Decimal("50000");                     │
                         │                                          │
                         │  CALL_METHOD                             │
                         │    Address("recipient_account")          │
                         │    "deposit_batch"                       │
                         │    Expression("ENTIRE_WORKTOP");         │
                         │                                          │
                         │  YIELD_TO_PARENT;                        │
                         └─────────────────────────────────────────┘
```

### Step-by-Step Flow

#### Phase 1: DAO Signature Collection

**Step 1: dApp Requests dao_withdraw Subintent from Orchestrator**

```
dApp (Frontend)                         Orchestrator (Backend)
      │                                        │
      ├─► GET /subintent/dao_withdraw         │
      │   { proposal_id: "123" }        ──────►│
      │                                        │
      │◄────────────────────────────────────── │
          { manifest: "CALL_METHOD..." }       │
```

**Step 2: dApp Sends to Wallet for Signing**

The dApp sends the manifest to the wallet via `sendPreAuthorizationRequest` with a 7-day expiry.

**Step 3: dApp Returns Signed Subintent to Orchestrator**

```
dApp (Frontend)                         Orchestrator (Backend)
      │                                        │
      ├─► POST /subintent/dao_withdraw/sign    │
      │   { signed_hex: "5e331f..." }   ──────►│
      │                                        ├─► Combines signatures
      │◄────────────────────────────────────── │
          { status: "ok", sigs: 3/4 }          │
```

**Step 4: Orchestrator Tracks Signature Progress**

```
Orchestrator Database
┌─────────────────────────────────────────────────────────────┐
│ proposal_id: "123"                                          │
├─────────────────────────────────────────────────────────────┤
│ dao_withdraw:                                               │
│   manifest: "CALL_METHOD..."                                │
│   required_sigs: 3                                          │
│   collected_sigs: 3 ✓  READY FOR SUBMISSION                 │
│   signed_partial_tx: "5e331f..."                            │
└─────────────────────────────────────────────────────────────┘
```

#### Phase 2: Fee Payment & Submission

Once DAO threshold is met, **anyone** can pay fees. The Orchestrator handles submission:
- **Fee payer**: Anyone who signs the main tx intent to authorize `lock_fee` from their account
- **Orchestrator**: Notarizes and submits the transaction to the network

**Step 5: Fee Payer Signs Main Transaction**

```
dApp (Frontend)                         Orchestrator (Backend)
      │                                        │
      ├─► POST /proposal/123/prepare           │
      │   { fee_payer: "account_rdx1..." }───► │
      │                                        │
      │                                        ├─► Builds main tx intent
      │                                        │   (with lock_fee from fee_payer)
      │                                        ├─► Adds dao_withdraw subintent
      │                                        ├─► Previews
      │                                        │
      │◄────────────────────────────────────── │
          { tx_to_sign: "..." }                │
      │                                        │
      │   [Wallet signs main tx intent]        │
      │                                        │
      ├─► POST /proposal/123/add-fee-signature │
      │   { signed_tx: "4d220e..." } ─────────►│
      │                                        │
      │◄────────────────────────────────────── │
          { status: "ready_for_submission" }   │
```

**Step 6: Orchestrator Notarizes & Submits**

```
dApp (Frontend)                         Orchestrator (Backend)
      │                                        │
      ├─► POST /proposal/123/submit ──────────►│
      │                                        │
      │                                        ├─► Notarizes & submits
      │                                        │
      │◄────────────────────────────────────── │
          { tx_id: "txid_rdx1..." }            │
```

#### Execution Order on Network

```
1. Main tx intent starts
   │
2. ├─► lock_fee(fee_payer_account, 100 XRD)
   │
3. ├─► YIELD_TO_CHILD("dao_withdraw")
   │       │
   │       ▼
   │   dao_withdraw subintent executes:
   │       ├─► withdraw(dao_treasury, 50000 tokens)
   │       ├─► deposit_batch(recipient, ENTIRE_WORKTOP)
   │       └─► YIELD_TO_PARENT
   │
4. └─► Main tx intent completes

5. Transaction committed ✓
```

#### Flow Summary

| Phase | Step | Actor | Action |
|-------|------|-------|--------|
| **1** | 1 | dApp | Requests dao_withdraw manifest from Orchestrator |
| | 2 | **Orchestrator** | Creates & returns manifest |
| | 3 | dApp | Sends to wallet via `sendPreAuthorizationRequest` |
| | 4 | Wallet | DAO member signs |
| | 5 | dApp | Posts signed hex to Orchestrator |
| | 6 | **Orchestrator** | Combines signatures until threshold met (3/4) |
| **2** | 7 | **Fee payer** | Requests main tx with their account for lock_fee |
| | 8 | **Orchestrator** | Builds main tx intent with lock_fee + dao_withdraw subintent |
| | 9 | **Fee payer** | Signs main tx intent (authorizes lock_fee) |
| | 10 | **Orchestrator** | Notarizes & submits to network |

### Why This Pattern?

| Aspect | Rationale |
|--------|-----------|
| **Single subintent** | Only the DAO action requires pre-authorization; fee locking is straightforward |
| **Fee in main intent** | `lock_fee` is called in the main tx intent, avoiding subintent complexity |
| **Decoupled signatures** | DAO signatures collected first; submitter signs main intent at submission time |
| **Anyone can submit** | Once DAO threshold is met, anyone willing to pay fees can complete the transaction |
| **Simple orchestration** | Main intent just locks fees and yields to the pre-signed DAO subintent |

## Security Considerations

- **Restrict signers to access rule members** — Only users defined in the DAO account's access rule should be allowed to sign proposals. This is critical because Radix transactions have a **maximum signature limit**, and allowing arbitrary users to sign could cause the subintent to exceed this limit, making it unvalidatable on-chain.
- **Expiry encoded in manifest** — Proposal expiry is set directly in the subintent manifest, not just tracked in the DApp. This ensures the network enforces expiry—it cannot be bypassed.
- **Dedicated key derivation** prevents attackers from leveraging public keys for malicious signing requests
- **Wallet stores signing public keys** returned from DApp for validity checks
- **Preview before submission** ensures proposals won't fail on-chain

### Proposal Validity is Not Cacheable

⚠️ **Important**: Collecting enough signatures does **not** mean the transaction is valid. The DApp must **always preview/validate at submission time**.

A previously "ready" proposal can become invalid in two scenarios:

1. **Access rule changes** — If another proposal modifies the DAO's access rules and is submitted first, pending proposals may no longer satisfy the new requirements. **Signatures must be re-collected** under the new rules.

   ```
   t+0: Proposal A reaches 3/4 signatures (valid under current rules)
   t+1: Proposal B (rule change) reaches threshold
   t+2: Proposal B submitted → DAO now requires 4/5 signatures
   t+3: Proposal A submission fails (only has 3 signatures, now needs 4)
   t+4: Proposal A must collect 1 more signature to become valid again
   ```

2. **Signature expiry** — Each signer's subintent signature has an expiry time encoded in the manifest. If a signature expires before submission, **that specific signature must be re-collected**. The proposal doesn't need all signatures again—just the expired one(s).

**Implementation requirements**:
- Preview the composed transaction immediately before submission
- Track individual signature expiry times
- When a signature expires or rules change, prompt affected signers to re-sign
- Handle the case where a "ready" proposal can no longer be executed

## References

- [Meeting notes (Jan 14)](../meetings/Multisig%20–%202026_01_14%2015_53%20CET%20–%20Notes%20by%20Gemini.md)
- [Foundation transition context](./foundation-transition-context.md)
- [Radix subintents reference](./radix-subintents-reference.md)

# Multisig Orchestrator

Radix DLT multisig transaction orchestrator — propose, collect signatures, and submit multi-party transactions on Babylon Stokenet.

## Prerequisites

- Docker & Docker Compose
- Rust (2021 edition) + Cargo
- Node.js + pnpm

## Quick Start

```bash
./dev.sh
```

This starts PostgreSQL, the backend, and the frontend in one command.

## Architecture

### How It Works

The server builds a transaction subintent from the user-provided manifest, and each signer approves via wallet pre-authorization. The server collects signatures, validates them against the account's on-ledger access rule, and — once the threshold is met — assembles a `NotarizedTransactionV2` with one child subintent (the withdrawal) and a main intent that pays fees from the server's own account, then submits it to the Radix Gateway.

### Signing Flow

```mermaid
sequenceDiagram
    participant Browser
    participant Wallet
    participant Server
    participant Gateway

    Note over Browser,Server: 1. Create Proposal
    Browser->>Server: POST /proposals {manifest_text, expiry_epoch}
    Server->>Gateway: GET current epoch
    Server->>Server: Compile manifest into unsigned PartialTransactionV2
    Server-->>Browser: Proposal (subintent_hash, status: created)

    Note over Browser,Wallet: 2. Collect Signatures (repeat per signer)
    Browser->>Wallet: sendPreAuthorizationRequest(manifest)
    Wallet-->>Browser: SignedPartialTransactionV2 hex
    Browser->>Server: POST /proposals/{id}/sign {signed_partial_transaction_hex}
    Server->>Server: Extract Ed25519 pubkey + signature from hex
    Server->>Server: Hash pubkey → key_hash
    Server->>Gateway: Fetch account access rule
    Server->>Server: Match key_hash against access rule signers
    Server-->>Browser: Signature stored (status: signing → ready when threshold met)

    Note over Browser,Gateway: 3. Submit Transaction
    Browser->>Server: POST /proposals/{id}/submit
    Server->>Server: Reconstruct withdrawal partial (stored unsigned + collected sigs)
    Server->>Server: Compose NotarizedTransactionV2 (1 child: withdrawal)
    Server->>Server: Notarize with fee payer key (pays fee in main manifest)
    Server->>Gateway: POST /transaction/submit
    Gateway-->>Server: Poll until committed
    Server-->>Browser: status: committed
```

### Transaction Structure

The final `NotarizedTransactionV2` contains one child subintent:

| Child | Manifest | Signed by |
|---|---|---|
| `withdrawal` | User's proposal manifest → `YIELD_TO_PARENT` | Multisig signers (collected during signing phase) |

The main intent manifest pays fees and yields to the child:

```
CALL_METHOD Address("<fee_payer_account>") "lock_fee" Decimal("10");
YIELD_TO_CHILD("withdrawal");
```

The server's fee payer key notarizes the transaction (`notary_is_signatory: true` — the key controls the fee payer account, authorising the `lock_fee` call).

## CLI Tools

### Generate Fee Payer Key

Generates an Ed25519 keypair for the server's fee payer account and optionally funds it via the Stokenet faucet.

```bash
cd generate-fee-payer-cli && cargo run
```

Outputs a ready-to-use `FEE_PAYER_PRIVATE_KEY_HEX=...` line for your `.env`.

### Create Multisig Account

Generates a transaction manifest for creating an n-of-m multisig account.

```bash
cd create-account-cli && cargo run
```

## Manual Start

### 1. Database (PostgreSQL)

```bash
docker compose up -d
```

Starts PostgreSQL 17 on port 5432. Credentials: `postgres/postgres`, database: `multisig_orchestrator`.

### 2. Backend (Rust/Axum)

```bash
cd multisig-server
cp .env.example .env   # first time only
cargo run
```

Runs on http://localhost:3001. Auto-applies database migrations on startup.

### 3. Frontend (React/TanStack Start)

```bash
cd multisig-app
pnpm install             # first time only
pnpm run dev
```

Runs on http://localhost:3000.

## Development

This project uses [pnpm](https://pnpm.io/) as the package manager with a workspace setup.

```bash
pnpm install              # install all dependencies
pnpm fmt                  # format all files (oxfmt)
pnpm fmt:check            # check formatting without writing
pnpm lint                 # lint JS/TS files (oxlint)
```

Pre-commit hooks (via Husky + lint-staged) automatically format and lint staged files.

## Shutdown

```bash
docker compose down     # stop PostgreSQL
```

`./dev.sh` handles graceful shutdown of all services on Ctrl+C.

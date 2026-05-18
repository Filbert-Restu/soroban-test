# 🏦 Soroban Escrow Contract

> An on-chain escrow system built on **Stellar Soroban** — transparent, trustless, and secure without a centralized intermediary.

---

## 📋 Table of Contents

- [About the Project](#about-the-project)
- [Key Features](#key-features)
- [Architecture & Lifecycle](#architecture--lifecycle)
- [Data Structures](#data-structures)
- [Contract Functions](#contract-functions)
- [Installation & Setup](#installation--setup)
- [How to Deploy](#how-to-deploy)
- [Running Tests](#running-tests)
- [CLI Usage Examples](#cli-usage-examples)
- [Security](#security)
- [Roadmap](#roadmap)

---

## About the Project

This contract enables two parties (buyer and seller) to transact with an automatic smart contract guarantor. Funds are locked inside the contract until conditions are met, eliminating fraud risk from either party.

**Real-world use cases:**
- Freelance service payments (buyer pays only when work is complete)
- Digital/physical goods transactions
- Rental services with deposits
- Conditional crowdfunding

---

## Key Features

| Feature | Description |
|---------|-------------|
| 🔒 **Locked Funds** | Tokens are locked in the contract and cannot be withdrawn arbitrarily |
| ⚖️ **Arbiter System** | A neutral third party decides in the event of a dispute |
| ⏰ **Auto-Refund** | Buyer can claim an automatic refund if the deadline is missed |
| 🏷️ **Multi-Token** | Supports all SAC tokens (XLM, USDC, etc.) |
| 🔍 **Flexible Queries** | Filter escrows by buyer, seller, or dispute status |
| 🛡️ **Strict Auth** | Every action validates the caller's identity via `require_auth()` |

---

## Architecture & Lifecycle

```
                        ┌─────────────────┐
                        │  create_escrow  │
                        │    (Buyer)      │
                        └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │     PENDING     │
                        └────────┬────────┘
                                 │ fund_escrow (Buyer)
                                 ▼
                        ┌─────────────────┐
                  ┌────►│     FUNDED      │◄────────────────┐
                  │     └────────┬────────┘                 │
                  │              │                          │
          deadline│      ┌───────┴────────┐                 │
           passed │      │                │ raise_dispute   │
                  │      ▼                ▼  (Buyer/Seller) │
                  │  release_funds  return_funds            │
                  │  (Buyer /       (Seller / Arbiter)      │
                  │   Arbiter)                              │
                  │      │                │                  │
                  │      ▼                ▼                  │
                  │  ┌──────────┐  ┌──────────────┐  ┌───────────┐
                  │  │COMPLETED │  │   REFUNDED   │  │ DISPUTED  │
                  │  └──────────┘  └──────────────┘  └─────┬─────┘
                  │                                         │
                  │                            Arbiter decides:
                  │                            release_funds / return_funds
                  │                                         │
                  └─────────────────────────────────────────┘
```

---

## Data Structures

### `EscrowStatus` (Enum)

```rust
pub enum EscrowStatus {
    Pending,    // Escrow created, not yet funded
    Funded,     // Funds locked in the contract
    Completed,  // Funds successfully released to seller
    Refunded,   // Funds returned to buyer
    Disputed,   // Under arbiter resolution process
}
```

### `Escrow` (Struct)

```rust
pub struct Escrow {
    pub id: u64,             // Unique auto-increment ID
    pub buyer: Address,      // Buyer's address
    pub seller: Address,     // Seller's address
    pub arbiter: Address,    // Arbiter's address (neutral party)
    pub token: Address,      // Token contract address (SAC)
    pub amount: i128,        // Amount in smallest token unit
    pub description: String, // Transaction description
    pub deadline: u64,       // UNIX timestamp deadline
    pub status: EscrowStatus,
}
```

---

## Contract Functions

### Write Functions

#### `create_escrow` → `u64`
Creates a new escrow. Funds are not transferred at this stage.
```
Parameters:
  buyer       : Address — Must call this function (require_auth)
  seller      : Address — Fund recipient if transaction succeeds
  arbiter     : Address — Dispute resolver if disagreement arises
  token       : Address — SAC token contract address to use
  amount      : i128    — Must be > 0
  description : String  — Transaction description
  deadline    : u64     — Must be in the future (> current ledger timestamp)

Returns: Newly created escrow ID
```

#### `fund_escrow` → `String`
Buyer transfers funds to the contract. Status: Pending → Funded.
```
Parameters:
  id : u64 — ID of the escrow to fund

Validations:
  - Status must be Pending
  - Deadline must not have passed
  - Buyer must have already approved token allowance to the contract
```

#### `release_funds` → `String`
Funds are sent to the seller. Status: Funded/Disputed → Completed.
```
Parameters:
  id     : u64     — Escrow ID
  caller : Address — Must be buyer or arbiter
```

#### `return_funds` → `String`
Funds are returned to the buyer. Status: Funded/Disputed → Refunded.
```
Parameters:
  id     : u64     — Escrow ID
  caller : Address — Must be seller or arbiter
```

#### `raise_dispute` → `String`
Escalates to the arbiter. Status: Funded → Disputed.
```
Parameters:
  id     : u64     — Escrow ID
  caller : Address — Must be buyer or seller
```

#### `claim_deadline_refund` → `String`
Buyer claims an automatic refund after the deadline has passed.
```
Parameters:
  id : u64 — Escrow ID (status must be Funded, deadline must have passed)
```

---

### Read Functions

| Function | Return | Description |
|----------|--------|-------------|
| `get_escrow(id)` | `Option<Escrow>` | Details of a single escrow |
| `get_all_escrows()` | `Vec<Escrow>` | All escrows |
| `get_escrows_by_buyer(addr)` | `Vec<Escrow>` | Filter by buyer |
| `get_escrows_by_seller(addr)` | `Vec<Escrow>` | Filter by seller |
| `get_disputed_escrows()` | `Vec<Escrow>` | Only those with Disputed status |

---

## Installation & Setup

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WebAssembly target
rustup target add wasm32-unknown-unknown

# Install Stellar CLI
cargo install --locked stellar-cli --features opt
```

### Clone & Build

```bash
git clone https://github.com/username/soroban-escrow
cd soroban-escrow

# Build contract
cargo build --target wasm32-unknown-unknown --release
```

The `.wasm` file will be available at:
```
target/wasm32-unknown-unknown/release/soroban_escrow.wasm
```

---

## How to Deploy

### 1. Setup Network (Testnet)

```bash
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

### 2. Create & Fund Account

```bash
# Generate a new keypair
stellar keys generate --global mywallet --network testnet

# Check address
stellar keys address mywallet

# Fund via Friendbot (testnet only)
curl "https://friendbot.stellar.org?addr=$(stellar keys address mywallet)"
```

### 3. Deploy Contract

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/soroban_escrow.wasm \
  --source mywallet \
  --network testnet
```

Save the **Contract ID** that is returned.

---

## Running Tests

```bash
# Run all tests
cargo test

# With detailed output
cargo test -- --nocapture

# Specific test
cargo test test_success_flow -- --nocapture
```

### Available Test Scenarios

```
✅ test_create_escrow           — Create escrow & validate data
✅ test_success_flow            — Create → Fund → Release funds to seller
✅ test_dispute_refund_flow     — Dispute → Arbiter refunds to buyer
✅ test_dispute_to_seller_flow  — Dispute → Arbiter pays to seller
✅ test_claim_deadline_refund   — Auto-refund after deadline
✅ test_query_filter            — Filter escrows by buyer/seller/status
```

---

## CLI Usage Examples

### Create Escrow

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source buyer_wallet \
  --network testnet \
  -- create_escrow \
  --buyer <BUYER_ADDRESS> \
  --seller <SELLER_ADDRESS> \
  --arbiter <ARBITER_ADDRESS> \
  --token <TOKEN_CONTRACT_ID> \
  --amount 100000000 \
  --description "Payment for logo design service" \
  --deadline 1800000000
```

### Fund Escrow

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source buyer_wallet \
  --network testnet \
  -- fund_escrow \
  --id 1
```

### Release Funds (Buyer confirms)

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source buyer_wallet \
  --network testnet \
  -- release_funds \
  --id 1 \
  --caller <BUYER_ADDRESS>
```

### Check Escrow Status

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source mywallet \
  --network testnet \
  -- get_escrow \
  --id 1
```

---

## Security

### Already Implemented

- **`require_auth()`** — Every write function validates the caller's identity
- **State Machine** — Status transitions are strictly validated; states cannot be skipped
- **Deadline Guard** — Funds cannot be locked into an expired escrow
- **Role Separation** — Buyer, seller, and arbiter each have distinct permissions

### Important Notes

> ⚠️ **The arbiter is a single point of trust.** Choose a trusted and neutral arbiter. Consider using multisig or a DAO for high-value cases.

> ⚠️ **Contract has not been audited.** Conduct a security audit before using on mainnet with real funds.

> ⚠️ **Instance storage** is used to store the escrow Vec. For large-scale use, consider refactoring to Persistent storage per-ID.

---

## Roadmap

- [ ] **Arbiter fee** — Arbiter receives a small percentage as compensation
- [ ] **Partial release** — Release funds partially (e.g., milestone-based payments)
- [ ] **Multisig arbiter** — Decisions require a majority from an arbiter panel
- [ ] **On-chain messaging** — Buyer/seller can leave messages/evidence
- [ ] **Freighter frontend** — Freighter wallet integration for a production UI

---

## Key Contracts
- CABWM2ZFHEHLMNNGXKJ3WGWN6NIYBGSGWJCGQFAHTTEIRRL7OPX4JRDB

---

## License

MIT License — free to use, modify, and distribute.
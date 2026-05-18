# 🏦 Soroban Escrow Contract

> Sistem rekening bersama (escrow) on-chain berbasis **Stellar Soroban** — transparan, trustless, dan aman tanpa perantara terpusat.

---

## 📋 Daftar Isi

- [Tentang Proyek](#tentang-proyek)
- [Fitur Utama](#fitur-utama)
- [Arsitektur & Siklus Hidup](#arsitektur--siklus-hidup)
- [Struktur Data](#struktur-data)
- [Fungsi Contract](#fungsi-contract)
- [Instalasi & Setup](#instalasi--setup)
- [Cara Deploy](#cara-deploy)
- [Menjalankan Test](#menjalankan-test)
- [Contoh Penggunaan CLI](#contoh-penggunaan-cli)
- [Keamanan](#keamanan)
- [Roadmap](#roadmap)

---

## Tentang Proyek

Contract ini memungkinkan dua pihak (buyer dan seller) untuk melakukan transaksi dengan penjamin otomatis berupa smart contract. Dana dikunci di dalam contract hingga kondisi terpenuhi, menghilangkan risiko penipuan dari salah satu pihak.

**Use Case nyata:**
- Pembayaran jasa freelance (buyer bayar hanya jika pekerjaan selesai)
- Transaksi jual beli barang digital/fisik
- Layanan sewa dengan deposit
- Crowdfund bersyarat

---

## Fitur Utama

| Fitur | Deskripsi |
|-------|-----------|
| 🔒 **Dana Terkunci** | Token dikunci di contract, tidak bisa diambil sembarangan |
| ⚖️ **Sistem Arbiter** | Pihak ketiga netral memutuskan jika ada sengketa |
| ⏰ **Auto-Refund** | Buyer bisa klaim refund otomatis jika deadline terlewat |
| 🏷️ **Multi-Token** | Mendukung semua token SAC (XLM, USDC, dsb.) |
| 🔍 **Query Fleksibel** | Filter escrow by buyer, seller, atau status sengketa |
| 🛡️ **Auth Ketat** | Setiap aksi divalidasi identitas pemanggil via `require_auth()` |

---

## Arsitektur & Siklus Hidup

```
                        ┌─────────────────┐
                        │   buat_escrow   │
                        │    (Buyer)      │
                        └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │    MENUNGGU     │
                        └────────┬────────┘
                                 │ danai_escrow (Buyer)
                                 ▼
                        ┌─────────────────┐
                  ┌────►│    DIDANAI      │◄────────────────┐
                  │     └────────┬────────┘                 │
                  │              │                          │
          deadline│      ┌───────┴────────┐                 │
           lewat  │      │                │ ajukan_sengketa  │
                  │      ▼                ▼  (Buyer/Seller) │
                  │  lepas_dana     kembalikan_dana         │
                  │  (Buyer /       (Seller / Arbiter)      │
                  │   Arbiter)                              │
                  │      │                │                  │
                  │      ▼                ▼                  │
                  │  ┌────────┐    ┌──────────────┐  ┌────────────┐
                  │  │SELESAI │    │ DIKEMBALIKAN │  │ SENGKETA  │
                  │  └────────┘    └──────────────┘  └─────┬──────┘
                  │                                         │
                  │                            Arbiter memutuskan:
                  │                            lepas_dana / kembalikan_dana
                  │                                         │
                  └─────────────────────────────────────────┘
```

---

## Struktur Data

### `StatusEscrow` (Enum)

```rust
pub enum StatusEscrow {
    Menunggu,      // Escrow dibuat, belum didanai
    Didanai,       // Dana sudah dikunci di contract
    Selesai,       // Dana berhasil dilepas ke seller
    Dikembalikan,  // Dana dikembalikan ke buyer
    Sengketa,      // Dalam proses resolusi arbiter
}
```

### `Escrow` (Struct)

```rust
pub struct Escrow {
    pub id: u64,            // ID unik auto-increment
    pub buyer: Address,     // Alamat pembeli
    pub seller: Address,    // Alamat penjual
    pub arbiter: Address,   // Alamat arbiter (pihak netral)
    pub token: Address,     // Alamat kontrak token (SAC)
    pub jumlah: i128,       // Jumlah dalam satuan terkecil token
    pub deskripsi: String,  // Keterangan transaksi
    pub deadline: u64,      // Timestamp UNIX batas waktu
    pub status: StatusEscrow,
}
```

---

## Fungsi Contract

### Write Functions

#### `buat_escrow` → `u64`
Membuat escrow baru. Dana belum ditransfer di tahap ini.
```
Parameter:
  buyer     : Address   — Harus memanggil fungsi ini (require_auth)
  seller    : Address   — Penerima dana jika transaksi sukses
  arbiter   : Address   — Pemutus sengketa jika ada perselisihan
  token     : Address   — Alamat token SAC yang digunakan
  jumlah    : i128      — Harus > 0
  deskripsi : String    — Keterangan transaksi
  deadline  : u64       — Harus di masa depan (> ledger timestamp saat ini)

Return: ID escrow yang baru dibuat
```

#### `danai_escrow` → `String`
Buyer mentransfer dana ke contract. Status: Menunggu → Didanai.
```
Parameter:
  id : u64 — ID escrow yang akan didanai

Validasi:
  - Status harus Menunggu
  - Deadline belum lewat
  - Buyer harus sudah approve token allowance ke contract
```

#### `lepas_dana` → `String`
Dana dikirim ke seller. Status: Didanai/Sengketa → Selesai.
```
Parameter:
  id     : u64     — ID escrow
  caller : Address — Harus buyer atau arbiter
```

#### `kembalikan_dana` → `String`
Dana dikembalikan ke buyer. Status: Didanai/Sengketa → Dikembalikan.
```
Parameter:
  id     : u64     — ID escrow
  caller : Address — Harus seller atau arbiter
```

#### `ajukan_sengketa` → `String`
Eskalasi ke arbiter. Status: Didanai → Sengketa.
```
Parameter:
  id     : u64     — ID escrow
  caller : Address — Harus buyer atau seller
```

#### `klaim_refund_deadline` → `String`
Buyer klaim refund otomatis setelah deadline lewat.
```
Parameter:
  id : u64 — ID escrow (status harus Didanai, deadline sudah lewat)
```

---

### Read Functions

| Fungsi | Return | Deskripsi |
|--------|--------|-----------|
| `get_escrow(id)` | `Option<Escrow>` | Detail satu escrow |
| `get_semua_escrow()` | `Vec<Escrow>` | Semua escrow |
| `get_escrow_by_buyer(addr)` | `Vec<Escrow>` | Filter by buyer |
| `get_escrow_by_seller(addr)` | `Vec<Escrow>` | Filter by seller |
| `get_escrow_sengketa()` | `Vec<Escrow>` | Hanya yang berstatus Sengketa |

---

## Instalasi & Setup

### Prasyarat

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Tambahkan target WebAssembly
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

File `.wasm` akan tersedia di:
```
target/wasm32-unknown-unknown/release/soroban_escrow.wasm
```

---

## Cara Deploy

### 1. Setup Network (Testnet)

```bash
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

### 2. Buat & Fund Akun

```bash
# Generate keypair baru
stellar keys generate --global mywallet --network testnet

# Cek alamat
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

Simpan **Contract ID** yang dikembalikan.

---

## Menjalankan Test

```bash
# Jalankan semua test
cargo test

# Dengan output detail
cargo test -- --nocapture

# Test spesifik
cargo test test_alur_sukses -- --nocapture
```

### Skenario Test yang Ada

```
✅ test_buat_escrow             — Membuat escrow & validasi data
✅ test_alur_sukses             — Buat → Danai → Lepas dana ke seller
✅ test_alur_sengketa_refund    — Sengketa → Arbiter refund ke buyer
✅ test_alur_sengketa_ke_seller — Sengketa → Arbiter bayar ke seller
✅ test_klaim_refund_deadline   — Auto-refund setelah deadline
✅ test_query_filter            — Filter escrow by buyer/seller/status
```

---

## Contoh Penggunaan CLI

### Buat Escrow

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source buyer_wallet \
  --network testnet \
  -- buat_escrow \
  --buyer <BUYER_ADDRESS> \
  --seller <SELLER_ADDRESS> \
  --arbiter <ARBITER_ADDRESS> \
  --token <TOKEN_CONTRACT_ID> \
  --jumlah 100000000 \
  --deskripsi "Pembayaran jasa desain logo" \
  --deadline 1800000000
```

### Danai Escrow

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source buyer_wallet \
  --network testnet \
  -- danai_escrow \
  --id 1
```

### Lepas Dana (Buyer konfirmasi)

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source buyer_wallet \
  --network testnet \
  -- lepas_dana \
  --id 1 \
  --caller <BUYER_ADDRESS>
```

### Cek Status Escrow

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source mywallet \
  --network testnet \
  -- get_escrow \
  --id 1
```

---

## Keamanan

### Yang Sudah Diimplementasi

- **`require_auth()`** — Setiap fungsi write memvalidasi identitas pemanggil
- **State Machine** — Transisi status divalidasi ketat; tidak bisa skip state
- **Deadline Guard** — Dana tidak bisa dikunci pada escrow yang sudah kedaluwarsa
- **Role Separation** — Buyer, seller, dan arbiter masing-masing punya hak berbeda

### Catatan Penting

> ⚠️ **Arbiter adalah single point of trust.** Pilih arbiter yang terpercaya dan netral. Pertimbangkan menggunakan multisig atau DAO untuk kasus nilai tinggi.

> ⚠️ **Contract belum diaudit.** Lakukan security audit sebelum digunakan di mainnet dengan dana nyata.

> ⚠️ **Instance storage** digunakan untuk menyimpan Vec escrow. Untuk skala besar, pertimbangkan refactor ke Persistent storage per-ID.

---

## Roadmap

- [ ] **Fee arbiter** — Arbiter mendapat persentase kecil sebagai kompensasi
- [ ] **Partial release** — Lepas dana sebagian (misal: milestone-based payment)
- [ ] **Multisig arbiter** — Keputusan memerlukan mayoritas dari panel arbiter
- [ ] **On-chain messaging** — Buyer/seller bisa meninggalkan pesan/bukti
- [ ] **Frontend Freighter** — Integrasi wallet Freighter untuk UI produksi

---

## Lisensi

MIT License — bebas digunakan, dimodifikasi, dan didistribusikan.
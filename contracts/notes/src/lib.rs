#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    token, Address, Env, String, Symbol, Vec,
};

// ============================================================
// TIPE DATA
// ============================================================

/// Status escrow yang menggambarkan siklus hidup transaksi
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum StatusEscrow {
    Menunggu,     // Escrow dibuat, belum didanai oleh buyer
    Didanai,      // Buyer sudah mentransfer dana ke contract
    Selesai,      // Dana berhasil dilepas ke seller
    Dikembalikan, // Dana dikembalikan ke buyer
    Sengketa,     // Salah satu pihak mengajukan sengketa
}

/// Struct utama yang menyimpan detail setiap transaksi escrow
#[contracttype]
#[derive(Clone, Debug)]
pub struct Escrow {
    pub id: u64,
    pub buyer: Address,   // Pihak yang membayar / pembeli
    pub seller: Address,  // Pihak yang menerima dana / penjual
    pub arbiter: Address, // Pihak ketiga netral untuk resolusi sengketa
    pub token: Address,   // Alamat token yang digunakan (misal: XLM, USDC)
    pub jumlah: i128,     // Jumlah dana dalam satuan terkecil token
    pub deskripsi: String,
    pub deadline: u64,    // Timestamp UNIX batas waktu konfirmasi buyer
    pub status: StatusEscrow,
}

// ============================================================
// STORAGE KEYS
// ============================================================

const ESCROW_LIST: Symbol = symbol_short!("ESC_LIST");  // Menyimpan Vec<Escrow>
const ESCROW_CNT: Symbol  = symbol_short!("ESC_CNT");   // Counter ID auto-increment

// ============================================================
// CONTRACT
// ============================================================

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {

    // ----------------------------------------------------------
    // BUAT ESCROW BARU
    // ----------------------------------------------------------
    /// Buyer membuat perjanjian escrow dengan menetapkan seller,
    /// arbiter, token yang digunakan, jumlah, dan batas waktu.
    /// Dana belum ditransfer di sini — buyer hanya mendaftarkan escrow.
    pub fn buat_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        arbiter: Address,
        token: Address,
        jumlah: i128,
        deskripsi: String,
        deadline: u64,
    ) -> u64 {
        // Pastikan yang memanggil adalah buyer itu sendiri
        buyer.require_auth();

        // Validasi jumlah harus lebih dari 0
        if jumlah <= 0 {
            panic!("Jumlah escrow harus lebih dari 0");
        }

        // Validasi deadline harus di masa depan
        let waktu_sekarang = env.ledger().timestamp();
        if deadline <= waktu_sekarang {
            panic!("Deadline harus di masa depan");
        }

        // Ambil dan increment counter ID
        let id: u64 = env.storage().instance().get(&ESCROW_CNT).unwrap_or(0u64) + 1;

        let escrow = Escrow {
            id,
            buyer,
            seller,
            arbiter,
            token,
            jumlah,
            deskripsi,
            deadline,
            status: StatusEscrow::Menunggu,
        };

        // Tambahkan ke daftar escrow
        let mut list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));
        list.push_back(escrow);

        env.storage().instance().set(&ESCROW_LIST, &list);
        env.storage().instance().set(&ESCROW_CNT, &id);

        // Kembalikan ID escrow yang baru dibuat
        id
    }

    // ----------------------------------------------------------
    // DANAI ESCROW
    // ----------------------------------------------------------
    /// Buyer mentransfer dana ke contract sebagai jaminan.
    /// Status berubah dari Menunggu → Didanai.
    pub fn danai_escrow(env: Env, id: u64) -> String {
        let mut list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));

        for i in 0..list.len() {
            let mut escrow = list.get(i).unwrap();

            if escrow.id == id {
                // Escrow harus dalam status Menunggu
                if escrow.status != StatusEscrow::Menunggu {
                    return String::from_str(&env, "Error: Escrow sudah didanai atau sudah selesai");
                }

                // Pastikan deadline belum lewat
                if env.ledger().timestamp() > escrow.deadline {
                    return String::from_str(&env, "Error: Deadline escrow sudah lewat");
                }

                // Autentikasi buyer
                escrow.buyer.require_auth();

                // Transfer token dari wallet buyer → contract
                token::Client::new(&env, &escrow.token).transfer(
                    &escrow.buyer,
                    &env.current_contract_address(),
                    &escrow.jumlah,
                );

                escrow.status = StatusEscrow::Didanai;
                list.set(i, escrow);
                env.storage().instance().set(&ESCROW_LIST, &list);

                return String::from_str(&env, "Sukses: Dana berhasil dikunci dalam escrow");
            }
        }

        String::from_str(&env, "Error: ID escrow tidak ditemukan")
    }

    // ----------------------------------------------------------
    // LEPAS DANA KE SELLER
    // ----------------------------------------------------------
    /// Buyer mengonfirmasi bahwa pekerjaan/barang sudah diterima,
    /// atau arbiter memutuskan dana layak diberikan ke seller.
    /// Status berubah → Selesai.
    pub fn lepas_dana(env: Env, id: u64, caller: Address) -> String {
        caller.require_auth();

        let mut list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));

        for i in 0..list.len() {
            let mut escrow = list.get(i).unwrap();

            if escrow.id == id {
                // Dana hanya bisa dilepas jika status Didanai atau Sengketa
                if escrow.status != StatusEscrow::Didanai
                    && escrow.status != StatusEscrow::Sengketa
                {
                    return String::from_str(&env, "Error: Escrow tidak dalam kondisi aktif");
                }

                // Hanya buyer atau arbiter yang berwenang melepas dana ke seller
                if caller != escrow.buyer && caller != escrow.arbiter {
                    return String::from_str(
                        &env,
                        "Error: Hanya buyer atau arbiter yang bisa melepas dana",
                    );
                }

                // Transfer token dari contract → seller
                token::Client::new(&env, &escrow.token).transfer(
                    &env.current_contract_address(),
                    &escrow.seller,
                    &escrow.jumlah,
                );

                escrow.status = StatusEscrow::Selesai;
                list.set(i, escrow);
                env.storage().instance().set(&ESCROW_LIST, &list);

                return String::from_str(&env, "Sukses: Dana berhasil dilepas ke seller");
            }
        }

        String::from_str(&env, "Error: ID escrow tidak ditemukan")
    }

    // ----------------------------------------------------------
    // KEMBALIKAN DANA KE BUYER
    // ----------------------------------------------------------
    /// Seller mengakui pekerjaan gagal/batal, atau arbiter memutuskan
    /// dana dikembalikan ke buyer.
    /// Status berubah → Dikembalikan.
    pub fn kembalikan_dana(env: Env, id: u64, caller: Address) -> String {
        caller.require_auth();

        let mut list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));

        for i in 0..list.len() {
            let mut escrow = list.get(i).unwrap();

            if escrow.id == id {
                // Dana hanya bisa dikembalikan jika status Didanai atau Sengketa
                if escrow.status != StatusEscrow::Didanai
                    && escrow.status != StatusEscrow::Sengketa
                {
                    return String::from_str(&env, "Error: Escrow tidak dalam kondisi aktif");
                }

                // Hanya seller atau arbiter yang berwenang mengembalikan dana
                if caller != escrow.seller && caller != escrow.arbiter {
                    return String::from_str(
                        &env,
                        "Error: Hanya seller atau arbiter yang bisa mengembalikan dana",
                    );
                }

                // Transfer token dari contract → buyer
                token::Client::new(&env, &escrow.token).transfer(
                    &env.current_contract_address(),
                    &escrow.buyer,
                    &escrow.jumlah,
                );

                escrow.status = StatusEscrow::Dikembalikan;
                list.set(i, escrow);
                env.storage().instance().set(&ESCROW_LIST, &list);

                return String::from_str(&env, "Sukses: Dana dikembalikan ke buyer");
            }
        }

        String::from_str(&env, "Error: ID escrow tidak ditemukan")
    }

    // ----------------------------------------------------------
    // AJUKAN SENGKETA
    // ----------------------------------------------------------
    /// Buyer atau seller mengajukan sengketa jika ada perselisihan.
    /// Arbiter kemudian akan memutuskan lewat lepas_dana atau kembalikan_dana.
    /// Status berubah → Sengketa.
    pub fn ajukan_sengketa(env: Env, id: u64, caller: Address) -> String {
        caller.require_auth();

        let mut list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));

        for i in 0..list.len() {
            let mut escrow = list.get(i).unwrap();

            if escrow.id == id {
                // Sengketa hanya bisa diajukan jika escrow sudah Didanai
                if escrow.status != StatusEscrow::Didanai {
                    return String::from_str(
                        &env,
                        "Error: Sengketa hanya bisa diajukan pada escrow yang sudah didanai",
                    );
                }

                // Hanya buyer atau seller yang bisa mengajukan sengketa
                if caller != escrow.buyer && caller != escrow.seller {
                    return String::from_str(
                        &env,
                        "Error: Hanya buyer atau seller yang bisa mengajukan sengketa",
                    );
                }

                escrow.status = StatusEscrow::Sengketa;
                list.set(i, escrow);
                env.storage().instance().set(&ESCROW_LIST, &list);

                return String::from_str(
                    &env,
                    "Sengketa diajukan: Arbiter akan memutuskan distribusi dana",
                );
            }
        }

        String::from_str(&env, "Error: ID escrow tidak ditemukan")
    }

    // ----------------------------------------------------------
    // KLAIM REFUND OTOMATIS (setelah deadline)
    // ----------------------------------------------------------
    /// Jika seller tidak memenuhi kewajiban dan deadline sudah lewat,
    /// buyer bisa klaim refund otomatis tanpa persetujuan seller/arbiter.
    pub fn klaim_refund_deadline(env: Env, id: u64) -> String {
        let mut list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));

        for i in 0..list.len() {
            let mut escrow = list.get(i).unwrap();

            if escrow.id == id {
                // Hanya bisa dilakukan jika status masih Didanai
                if escrow.status != StatusEscrow::Didanai {
                    return String::from_str(&env, "Error: Escrow tidak dalam kondisi aktif");
                }

                // Pastikan deadline sudah lewat
                if env.ledger().timestamp() <= escrow.deadline {
                    return String::from_str(&env, "Error: Deadline belum lewat");
                }

                // Autentikasi — hanya buyer yang bisa klaim
                escrow.buyer.require_auth();

                // Kembalikan dana ke buyer
                token::Client::new(&env, &escrow.token).transfer(
                    &env.current_contract_address(),
                    &escrow.buyer,
                    &escrow.jumlah,
                );

                escrow.status = StatusEscrow::Dikembalikan;
                list.set(i, escrow);
                env.storage().instance().set(&ESCROW_LIST, &list);

                return String::from_str(
                    &env,
                    "Sukses: Dana dikembalikan otomatis karena deadline terlewat",
                );
            }
        }

        String::from_str(&env, "Error: ID escrow tidak ditemukan")
    }

    // ----------------------------------------------------------
    // QUERY FUNCTIONS
    // ----------------------------------------------------------

    /// Ambil detail satu escrow berdasarkan ID
    pub fn get_escrow(env: Env, id: u64) -> Option<Escrow> {
        let list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));

        for i in 0..list.len() {
            let escrow = list.get(i).unwrap();
            if escrow.id == id {
                return Some(escrow);
            }
        }
        None
    }

    /// Ambil semua escrow (untuk admin / debugging)
    pub fn get_semua_escrow(env: Env) -> Vec<Escrow> {
        env.storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env))
    }

    /// Ambil semua escrow milik buyer tertentu
    pub fn get_escrow_by_buyer(env: Env, buyer: Address) -> Vec<Escrow> {
        let list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));
        let mut hasil = Vec::new(&env);

        for i in 0..list.len() {
            let escrow = list.get(i).unwrap();
            if escrow.buyer == buyer {
                hasil.push_back(escrow);
            }
        }
        hasil
    }

    /// Ambil semua escrow milik seller tertentu
    pub fn get_escrow_by_seller(env: Env, seller: Address) -> Vec<Escrow> {
        let list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));
        let mut hasil = Vec::new(&env);

        for i in 0..list.len() {
            let escrow = list.get(i).unwrap();
            if escrow.seller == seller {
                hasil.push_back(escrow);
            }
        }
        hasil
    }

    /// Ambil semua escrow yang sedang dalam status Sengketa
    /// (berguna untuk dashboard arbiter)
    pub fn get_escrow_sengketa(env: Env) -> Vec<Escrow> {
        let list: Vec<Escrow> = env
            .storage()
            .instance()
            .get(&ESCROW_LIST)
            .unwrap_or(Vec::new(&env));
        let mut hasil = Vec::new(&env);

        for i in 0..list.len() {
            let escrow = list.get(i).unwrap();
            if escrow.status == StatusEscrow::Sengketa {
                hasil.push_back(escrow);
            }
        }
        hasil
    }
}

mod test;
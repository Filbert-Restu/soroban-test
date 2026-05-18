#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger},
        token, Address, Env, IntoVal, String,
    };

    use crate::{EscrowContract, EscrowContractClient, StatusEscrow};

    // Helper: setup environment + deploy contract + token
    fn setup() -> (Env, EscrowContractClient<'static>, Address, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy contract escrow
        let contract_id = env.register_contract(None, EscrowContract);
        let client = EscrowContractClient::new(&env, &contract_id);

        // Buat alamat palsu untuk testing
        let buyer   = Address::generate(&env);
        let seller  = Address::generate(&env);
        let arbiter = Address::generate(&env);

        // Deploy token SAC untuk testing (simulasi XLM/USDC)
        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract(token_admin.clone());
        let token_client = token::StellarAssetClient::new(&env, &token_id);

        // Beri buyer saldo awal 1000 token (dalam stroops: 1000 * 1_000_000 = 1_000_000_000)
        token_client.mint(&buyer, &1_000_000_000i128);

        (env, client, buyer, seller, arbiter, token_id, contract_id)
    }

    // ============================================================
    // TEST 1: Buat escrow berhasil
    // ============================================================
    #[test]
    fn test_buat_escrow() {
        let (env, client, buyer, seller, arbiter, token_id, _) = setup();

        // Set ledger timestamp
        env.ledger().set_timestamp(1_000_000);

        let id = client.buat_escrow(
            &buyer,
            &seller,
            &arbiter,
            &token_id,
            &100_000_000i128,  // 100 token
            &String::from_str(&env, "Pembayaran jasa pembuatan website"),
            &2_000_000u64,     // deadline di masa depan
        );

        assert_eq!(id, 1, "ID escrow pertama harus 1");

        let escrow = client.get_escrow(&1u64).unwrap();
        assert_eq!(escrow.buyer, buyer);
        assert_eq!(escrow.seller, seller);
        assert_eq!(escrow.jumlah, 100_000_000i128);
        assert_eq!(escrow.status, StatusEscrow::Menunggu);
    }

    // ============================================================
    // TEST 2: Alur sukses — buat → danai → lepas dana
    // ============================================================
    #[test]
    fn test_alur_sukses() {
        let (env, client, buyer, seller, arbiter, token_id, contract_id) = setup();

        env.ledger().set_timestamp(1_000_000);

        let jumlah = 200_000_000i128; // 200 token

        // 1. Buat escrow
        let id = client.buat_escrow(
            &buyer,
            &seller,
            &arbiter,
            &token_id,
            &jumlah,
            &String::from_str(&env, "Pembelian laptop second"),
            &2_000_000u64,
        );

        // 2. Buyer mendanai escrow
        let res_danai = client.danai_escrow(&id);
        assert!(res_danai.to_string().contains("Sukses"));

        // Cek status berubah ke Didanai
        let escrow = client.get_escrow(&id).unwrap();
        assert_eq!(escrow.status, StatusEscrow::Didanai);

        // Cek saldo contract menerima dana
        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(token_client.balance(&contract_id), jumlah);

        // 3. Buyer melepas dana ke seller setelah barang diterima
        let res_lepas = client.lepas_dana(&id, &buyer);
        assert!(res_lepas.to_string().contains("Sukses"));

        // Cek status berubah ke Selesai
        let escrow = client.get_escrow(&id).unwrap();
        assert_eq!(escrow.status, StatusEscrow::Selesai);

        // Cek saldo seller bertambah
        assert_eq!(token_client.balance(&seller), jumlah);
        // Cek contract sudah kosong
        assert_eq!(token_client.balance(&contract_id), 0i128);
    }

    // ============================================================
    // TEST 3: Alur sengketa — arbiter memutuskan refund ke buyer
    // ============================================================
    #[test]
    fn test_alur_sengketa_refund() {
        let (env, client, buyer, seller, arbiter, token_id, contract_id) = setup();

        env.ledger().set_timestamp(1_000_000);

        let jumlah = 150_000_000i128;

        // Buat & danai escrow
        let id = client.buat_escrow(
            &buyer,
            &seller,
            &arbiter,
            &token_id,
            &jumlah,
            &String::from_str(&env, "Sewa jasa desainer"),
            &2_000_000u64,
        );
        client.danai_escrow(&id);

        // Buyer tidak puas → ajukan sengketa
        let res_sengketa = client.ajukan_sengketa(&id, &buyer);
        assert!(res_sengketa.to_string().contains("Sengketa diajukan"));

        let escrow = client.get_escrow(&id).unwrap();
        assert_eq!(escrow.status, StatusEscrow::Sengketa);

        // Arbiter memutuskan mengembalikan dana ke buyer
        let res_refund = client.kembalikan_dana(&id, &arbiter);
        assert!(res_refund.to_string().contains("Sukses"));

        // Cek buyer menerima dana kembali (saldo awal 1_000_000_000 - jumlah + jumlah = 1_000_000_000)
        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(token_client.balance(&buyer), 1_000_000_000i128);
        assert_eq!(token_client.balance(&contract_id), 0i128);
    }

    // ============================================================
    // TEST 4: Alur sengketa — arbiter melepas dana ke seller
    // ============================================================
    #[test]
    fn test_alur_sengketa_ke_seller() {
        let (env, client, buyer, seller, arbiter, token_id, _) = setup();

        env.ledger().set_timestamp(1_000_000);

        let jumlah = 50_000_000i128;

        let id = client.buat_escrow(
            &buyer, &seller, &arbiter, &token_id, &jumlah,
            &String::from_str(&env, "Pembayaran proyek Soroban"), &2_000_000u64,
        );
        client.danai_escrow(&id);

        // Seller mengajukan sengketa (merasa sudah kerja tapi buyer tidak mau bayar)
        client.ajukan_sengketa(&id, &seller);

        // Arbiter memutuskan pekerjaan layak dibayar → lepas ke seller
        let res = client.lepas_dana(&id, &arbiter);
        assert!(res.to_string().contains("Sukses"));

        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(token_client.balance(&seller), jumlah);
    }

    // ============================================================
    // TEST 5: Klaim refund otomatis setelah deadline
    // ============================================================
    #[test]
    fn test_klaim_refund_deadline() {
        let (env, client, buyer, seller, arbiter, token_id, _) = setup();

        // Timestamp awal sebelum deadline
        env.ledger().set_timestamp(1_000_000);

        let id = client.buat_escrow(
            &buyer, &seller, &arbiter, &token_id,
            &100_000_000i128,
            &String::from_str(&env, "Freelance dengan deadline ketat"),
            &1_500_000u64,   // deadline = 1.5 juta
        );
        client.danai_escrow(&id);

        // Majukan waktu melewati deadline
        env.ledger().set_timestamp(2_000_000);

        let res = client.klaim_refund_deadline(&id);
        assert!(res.to_string().contains("Sukses"));

        let escrow = client.get_escrow(&id).unwrap();
        assert_eq!(escrow.status, StatusEscrow::Dikembalikan);
    }

    // ============================================================
    // TEST 6: Query filter berdasarkan buyer / seller / sengketa
    // ============================================================
    #[test]
    fn test_query_filter() {
        let (env, client, buyer, seller, arbiter, token_id, _) = setup();
        env.ledger().set_timestamp(1_000_000);

        let buyer2 = Address::generate(&env);

        // Buat 3 escrow: 2 dari buyer, 1 dari buyer2
        client.buat_escrow(
            &buyer, &seller, &arbiter, &token_id, &10_000_000i128,
            &String::from_str(&env, "Escrow 1"), &2_000_000u64,
        );
        client.buat_escrow(
            &buyer, &seller, &arbiter, &token_id, &20_000_000i128,
            &String::from_str(&env, "Escrow 2"), &2_000_000u64,
        );
        client.buat_escrow(
            &buyer2, &seller, &arbiter, &token_id, &30_000_000i128,
            &String::from_str(&env, "Escrow 3"), &2_000_000u64,
        );

        // Filter by buyer
        let milik_buyer = client.get_escrow_by_buyer(&buyer);
        assert_eq!(milik_buyer.len(), 2);

        // Filter by seller (semua)
        let milik_seller = client.get_escrow_by_seller(&seller);
        assert_eq!(milik_seller.len(), 3);

        // Danai escrow 1 lalu ajukan sengketa
        let token_admin = Address::generate(&env);
        let token_asset = token::StellarAssetClient::new(&env, &token_id);
        token_asset.mint(&buyer2, &1_000_000_000i128);

        client.danai_escrow(&1u64);
        client.ajukan_sengketa(&1u64, &buyer);

        let sengketa = client.get_escrow_sengketa();
        assert_eq!(sengketa.len(), 1);
        assert_eq!(sengketa.get(0).unwrap().id, 1u64);
    }
}
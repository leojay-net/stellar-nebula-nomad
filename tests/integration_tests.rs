#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{vec, Address, BytesN, Env, IntoVal, Val, Vec};
use stellar_nebula_nomad::{
    CellType, NebulaNomadContract, NebulaNomadContractClient, NebulaCell, NebulaLayout, Rarity,
    GRID_SIZE, TOTAL_CELLS,
};

fn setup_env() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_700_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let player = Address::generate(&env);
    (env, client, player)
}

// ─── generate_nebula_layout ───────────────────────────────────────────────

#[test]
fn test_generate_layout_dimensions() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[1u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout.width, GRID_SIZE);
    assert_eq!(layout.height, GRID_SIZE);
    assert_eq!(layout.cells.len(), TOTAL_CELLS);
}

#[test]
fn test_generate_layout_has_energy() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[42u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert!(layout.total_energy > 0);
}

#[test]
fn test_generate_layout_deterministic() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[7u8; 32]);
    let layout1 = client.generate_nebula_layout(&seed, &player);
    let layout2 = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout1.total_energy, layout2.total_energy);
    assert_eq!(layout1.seed, layout2.seed);
    assert_eq!(layout1.timestamp, layout2.timestamp);
}

#[test]
fn test_different_seeds_produce_different_layouts() {
    let (env, client, player) = setup_env();
    let seed_a = BytesN::from_array(&env, &[1u8; 32]);
    let seed_b = BytesN::from_array(&env, &[2u8; 32]);
    let layout_a = client.generate_nebula_layout(&seed_a, &player);
    let layout_b = client.generate_nebula_layout(&seed_b, &player);
    assert_ne!(layout_a.total_energy, layout_b.total_energy);
}

#[test]
fn test_layout_changes_with_ledger_state() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let player = Address::generate(&env);
    let seed = BytesN::from_array(&env, &[5u8; 32]);

    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    let layout1 = client.generate_nebula_layout(&seed, &player);

    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 200,
        timestamp: 2_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    let layout2 = client.generate_nebula_layout(&seed, &player);

    assert_ne!(layout1.total_energy, layout2.total_energy);
}

#[test]
fn test_layout_cell_coordinates() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[10u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);

    for i in 0..layout.cells.len() {
        let cell = layout.cells.get(i).unwrap();
        assert!(cell.x < GRID_SIZE);
        assert!(cell.y < GRID_SIZE);
    }
}

#[test]
fn test_layout_records_timestamp() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[3u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout.timestamp, 1_700_000_000);
}

#[test]
fn test_zero_seed_works() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[0u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout.cells.len(), TOTAL_CELLS);
}

// ─── calculate_rarity_tier ────────────────────────────────────────────────

fn make_layout(env: &Env, rare_count: u32, energy_per_cell: u32) -> NebulaLayout {
    let mut cells = Vec::new(env);
    let mut total_energy = 0u32;
    for i in 0..TOTAL_CELLS {
        let (cell_type, energy) = if i < rare_count {
            (CellType::Wormhole, 60 + energy_per_cell)
        } else {
            (CellType::Empty, energy_per_cell)
        };
        total_energy += energy;
        cells.push_back(NebulaCell {
            x: i % GRID_SIZE,
            y: i / GRID_SIZE,
            cell_type,
            energy,
        });
    }
    NebulaLayout {
        width: GRID_SIZE,
        height: GRID_SIZE,
        cells,
        seed: BytesN::from_array(env, &[0u8; 32]),
        timestamp: 0,
        total_energy,
    }
}

#[test]
fn test_rarity_common() {
    let (env, client, _) = setup_env();
    let layout = make_layout(&env, 0, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Common);
}

#[test]
fn test_rarity_uncommon() {
    let (env, client, _) = setup_env();
    // 5 rare cells × 10 = 50, energy_density ≈ 0 → score 50 → Uncommon
    let layout = make_layout(&env, 5, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Uncommon);
}

#[test]
fn test_rarity_rare() {
    let (env, client, _) = setup_env();
    // 10 rare cells × 10 = 100 → score 100 → Rare
    let layout = make_layout(&env, 10, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Rare);
}

#[test]
fn test_rarity_epic() {
    let (env, client, _) = setup_env();
    // 15 rare cells × 10 = 150 → score 150 → Epic
    let layout = make_layout(&env, 15, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Epic);
}

#[test]
fn test_rarity_legendary() {
    let (env, client, _) = setup_env();
    // 20 rare cells × 10 = 200 → score 200 → Legendary
    let layout = make_layout(&env, 20, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Legendary);
}

#[test]
fn test_rarity_energy_density_contributes() {
    let (env, client, _) = setup_env();
    // 4 rare cells × 10 = 40, with high energy per cell to push into Uncommon
    // energy_per_cell = 10 → total = 256 * 10 = 2560, density = 10 → score = 50
    let layout = make_layout(&env, 4, 10);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Uncommon);
}

#[test]
fn test_rarity_from_generated_layout() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[99u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    let rarity = client.calculate_rarity_tier(&layout);
    // Should be one of the valid rarity tiers
    assert!(
        rarity == Rarity::Common
            || rarity == Rarity::Uncommon
            || rarity == Rarity::Rare
            || rarity == Rarity::Epic
            || rarity == Rarity::Legendary
    );
}

// ─── scan_nebula (end-to-end + event emission) ───────────────────────────

#[test]
fn test_scan_nebula_returns_layout_and_rarity() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[50u8; 32]);
    let (layout, rarity) = client.scan_nebula(&seed, &player);
    assert_eq!(layout.width, GRID_SIZE);
    assert_eq!(layout.height, GRID_SIZE);
    assert_eq!(layout.cells.len(), TOTAL_CELLS);
    assert!(
        rarity == Rarity::Common
            || rarity == Rarity::Uncommon
            || rarity == Rarity::Rare
            || rarity == Rarity::Epic
            || rarity == Rarity::Legendary
    );
}

#[test]
fn test_scan_nebula_emits_event() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[77u8; 32]);
    let _result = client.scan_nebula(&seed, &player);

    let events = env.events().all();
    assert!(!events.is_empty(), "Expected NebulaScanned event to be emitted");

    // Verify the last event has the correct topics
    let last = events.get(events.len() - 1).unwrap();
    let (_contract_addr, topics, _data) = last;
    assert_eq!(topics.len(), 2);
}

#[test]
fn test_scan_nebula_consistency_with_individual_calls() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[33u8; 32]);

    let layout = client.generate_nebula_layout(&seed, &player);
    let rarity = client.calculate_rarity_tier(&layout);

    let (scan_layout, scan_rarity) = client.scan_nebula(&seed, &player);

    assert_eq!(layout.total_energy, scan_layout.total_energy);
    assert_eq!(rarity, scan_rarity);
}

use stellar_nebula_nomad::nebula_gen::{
    AnomalyType, NebulaError, NebulaGen, NebulaGenClient, ResourceClass,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn setup_env() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let cid = env.register_contract(None, NebulaGen);
    let admin = Address::generate(&env);
    NebulaGenClient::new(&env, &cid).init(&admin, &10u32, &8u32, &16u32);
    (env, cid, admin)
}

/// Build a non-zero 32-byte seed deterministically from a u64 value.
fn seed_from_u64(env: &Env, v: u64) -> BytesN<32> {
    let b = v.to_le_bytes();
    let mut arr = [0u8; 32];
    for i in 0..32usize {
        arr[i] = b[i % 8] ^ (i as u8).wrapping_add(1);
    }
    BytesN::from_array(env, &arr)
}

fn advance_ledgers(env: &Env, n: u32) {
    let seq = env.ledger().sequence();
    let ts = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        sequence_number: seq + n,
        timestamp: ts + (n as u64 * 5),
        protocol_version: 20,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 6_312_000,
    });
}

// ─── Initialisation ───────────────────────────────────────────────────────────

#[test]
fn test_init_stores_config() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let cfg = client.get_config().unwrap();
    assert_eq!(cfg.default_size, 10);
    assert_eq!(cfg.min_size, 8);
    assert_eq!(cfg.max_size, 16);
}

#[test]
fn test_double_init_rejected() {
    let (env, cid, admin) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let err = client.try_init(&admin, &10u32, &8u32, &16u32);
    assert_eq!(err, Err(Ok(NebulaError::AlreadyInitialized)));
}

#[test]
fn test_invalid_size_params_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let cid = env.register_contract(None, NebulaGen);
    let client = NebulaGenClient::new(&env, &cid);
    let admin = Address::generate(&env);
    // min_size = 0 should fail
    let err = client.try_init(&admin, &10u32, &0u32, &16u32);
    assert_eq!(err, Err(Ok(NebulaError::InvalidSize)));
}

// ─── Determinism ─────────────────────────────────────────────────────────────

#[test]
fn test_same_inputs_same_layout() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let seed = seed_from_u64(&env, 0xdeadbeef_cafecafe);

    // Generate layout for ship 1
    let layout1 = client.generate_nebula_layout(&player, &1u64, &42u64, &seed);

    // Reset ledger to initial state so env.ledger().sequence() / timestamp() are identical
    env.ledger().set(LedgerInfo {
        sequence_number: 0,
        timestamp: 0,
        protocol_version: 20,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 6_312_000,
    });

    // Generate for ship 2 — different ship_id, so hash must differ
    let layout2 = client.generate_nebula_layout(&player, &2u64, &42u64, &seed);
    assert_ne!(layout1.layout_hash, layout2.layout_hash);

    // Reset again and regenerate ship 1 — must produce identical hash
    env.ledger().set(LedgerInfo {
        sequence_number: 0,
        timestamp: 0,
        protocol_version: 20,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 6_312_000,
    });
    let layout1b = client.generate_nebula_layout(&player, &1u64, &42u64, &seed);
    assert_eq!(layout1.layout_hash, layout1b.layout_hash);
}

#[test]
fn test_different_regions_different_layouts() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let seed = seed_from_u64(&env, 12345);

    let l1 = client.generate_nebula_layout(&player, &1u64, &100u64, &seed);
    let l2 = client.generate_nebula_layout(&player, &1u64, &101u64, &seed);
    assert_ne!(l1.layout_hash, l2.layout_hash);
}

#[test]
fn test_different_ships_different_layouts() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let seed = seed_from_u64(&env, 99999);

    let l1 = client.generate_nebula_layout(&player, &1u64, &42u64, &seed);
    let l2 = client.generate_nebula_layout(&player, &2u64, &42u64, &seed);
    assert_ne!(l1.layout_hash, l2.layout_hash);
}

#[test]
fn test_different_seeds_different_layouts() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);

    let l1 = client.generate_nebula_layout(&player, &1u64, &42u64, &seed_from_u64(&env, 1));
    let l2 = client.generate_nebula_layout(&player, &1u64, &42u64, &seed_from_u64(&env, 2));
    assert_ne!(l1.layout_hash, l2.layout_hash);
}

// ─── Layout validity ──────────────────────────────────────────────────────────

#[test]
fn test_layout_has_correct_size() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let layout = client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 1));
    assert_eq!(layout.size, 10);
    assert_eq!(layout.anomalies.len(), 10);
}

#[test]
fn test_all_anomaly_coordinates_in_range() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let layout = client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 7));
    for i in 0..layout.size {
        let a = layout.anomalies.get(i).unwrap();
        assert!(a.x < 1000, "x={} out of range", a.x);
        assert!(a.y < 1000, "y={} out of range", a.y);
    }
}

#[test]
fn test_all_rarity_scores_in_range() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let layout = client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 8));
    for i in 0..layout.size {
        let a = layout.anomalies.get(i).unwrap();
        assert!(a.rarity <= 100, "rarity={} out of range", a.rarity);
    }
}

#[test]
fn test_resource_class_matches_rarity() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let layout = client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 9));
    for i in 0..layout.size {
        let a = layout.anomalies.get(i).unwrap();
        let expected = if a.rarity <= 33 {
            ResourceClass::Sparse
        } else if a.rarity <= 66 {
            ResourceClass::Moderate
        } else {
            ResourceClass::Abundant
        };
        assert_eq!(a.resource_class, expected);
    }
}

// ─── Query functions ──────────────────────────────────────────────────────────

#[test]
fn test_query_anomaly_returns_correct_data() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let layout = client.generate_nebula_layout(&player, &5u64, &99u64, &seed_from_u64(&env, 42));

    for i in 0..layout.size {
        let via_query = client.query_anomaly(&5u64, &i);
        let via_layout = layout.anomalies.get(i).unwrap();
        assert_eq!(via_query.x, via_layout.x);
        assert_eq!(via_query.y, via_layout.y);
        assert_eq!(via_query.rarity, via_layout.rarity);
    }
}

#[test]
fn test_query_out_of_bounds_fails() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 1));
    let err = client.try_query_anomaly(&1u64, &999u32);
    assert_eq!(err, Err(Ok(NebulaError::InvalidIndex)));
}

#[test]
fn test_query_without_layout_fails() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    // ship 999 has never generated a layout
    let err = client.try_query_anomaly(&999u64, &0u32);
    assert_eq!(err, Err(Ok(NebulaError::LayoutNotFound)));
}

#[test]
fn test_has_anomaly_returns_true_for_valid_indices() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let layout = client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 1));
    for i in 0..layout.size {
        assert!(client.has_anomaly(&1u64, &i));
    }
}

#[test]
fn test_has_anomaly_returns_false_for_out_of_bounds() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 1));
    assert!(!client.has_anomaly(&1u64, &10u32)); // size = 10, index 10 is OOB
    assert!(!client.has_anomaly(&1u64, &999u32));
}

#[test]
fn test_has_anomaly_false_when_no_layout() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    assert!(!client.has_anomaly(&42u64, &0u32));
}

#[test]
fn test_get_layout_returns_none_when_missing() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    assert!(client.get_layout(&99u64).is_none());
}

#[test]
fn test_new_scan_overwrites_previous_layout() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let l1 = client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 1));
    let l2 = client.generate_nebula_layout(&player, &1u64, &2u64, &seed_from_u64(&env, 2));
    // Active layout for ship 1 should now be l2
    let active = client.get_layout(&1u64).unwrap();
    assert_eq!(active.layout_hash, l2.layout_hash);
    assert_ne!(active.layout_hash, l1.layout_hash);
}

// ─── Error cases ──────────────────────────────────────────────────────────────

#[test]
fn test_all_zero_seed_rejected() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let zero_seed = BytesN::from_array(&env, &[0u8; 32]);
    let err = client.try_generate_nebula_layout(&player, &1u64, &42u64, &zero_seed);
    assert_eq!(err, Err(Ok(NebulaError::InvalidSeed)));
}

// ─── Admin ────────────────────────────────────────────────────────────────────

#[test]
fn test_update_default_size() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    client.update_default_size(&12u32);
    assert_eq!(client.get_config().unwrap().default_size, 12);
}

#[test]
fn test_update_default_size_out_of_range_rejected() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let err = client.try_update_default_size(&100u32); // > max_size of 16
    assert_eq!(err, Err(Ok(NebulaError::InvalidSize)));
}

#[test]
fn test_new_size_applies_to_next_generation() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);

    client.update_default_size(&16u32);
    let layout = client.generate_nebula_layout(&player, &1u64, &1u64, &seed_from_u64(&env, 1));
    assert_eq!(layout.size, 16);
    assert_eq!(layout.anomalies.len(), 16);
}

// ─── Ledger mixing (unpredictability) ────────────────────────────────────────

#[test]
fn test_same_inputs_different_ledger_different_layout() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);
    let seed = seed_from_u64(&env, 0xabcdef);

    // Generate at ledger 0
    let l1 = client.generate_nebula_layout(&player, &1u64, &1u64, &seed);

    // Advance ledger then regenerate with identical args
    advance_ledgers(&env, 100);
    let l2 = client.generate_nebula_layout(&player, &1u64, &1u64, &seed);

    // Ledger mixin must produce a different layout despite identical caller args
    assert_ne!(l1.layout_hash, l2.layout_hash);
}

// ─── 50-generation simulation ─────────────────────────────────────────────────

#[test]
fn test_fifty_generation_simulation() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);

    let mut hashes = soroban_sdk::Vec::<BytesN<32>>::new(&env);

    for region in 0u64..50 {
        let seed = seed_from_u64(&env, region.wrapping_mul(0x1234_5678) ^ 0xdeadbeef);
        let layout = client.generate_nebula_layout(&player, &region, &region, &seed);

        // Structural validity
        assert_eq!(layout.size, 10);
        assert_eq!(layout.anomalies.len(), 10);
        assert_eq!(layout.generated_at, env.ledger().timestamp());

        for i in 0..layout.size {
            let a = layout.anomalies.get(i).unwrap();
            assert!(a.x < 1000);
            assert!(a.y < 1000);
            assert!(a.rarity <= 100);
        }

        // `has_anomaly` and `query_anomaly` are consistent with the returned layout
        assert!(client.has_anomaly(&region, &0u32));
        assert!(client.has_anomaly(&region, &9u32));
        assert!(!client.has_anomaly(&region, &10u32));
        let a0_direct = layout.anomalies.get(0u32).unwrap();
        let a0_query  = client.query_anomaly(&region, &0u32);
        assert_eq!(a0_direct.x, a0_query.x);
        assert_eq!(a0_direct.y, a0_query.y);

        hashes.push_back(layout.layout_hash);

        // Advance ledger slightly between runs to simulate real-world spacing
        advance_ledgers(&env, 5);
    }

    // All 50 layout hashes must be unique
    for i in 0..50u32 {
        for j in (i + 1)..50u32 {
            assert_ne!(
                hashes.get(i).unwrap(),
                hashes.get(j).unwrap(),
                "collision at regions {} and {}",
                i,
                j
            );
        }
    }
}

// ─── Fuzz: varied anomaly type coverage ──────────────────────────────────────

#[test]
fn test_anomaly_type_coverage_across_layouts() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);

    let mut saw_dust     = false;
    let mut saw_ion      = false;
    let mut saw_crystal  = false;
    let mut saw_plasma   = false;
    let mut saw_dark     = false;

    for region in 0u64..20 {
        let seed = seed_from_u64(&env, region.wrapping_add(1).wrapping_mul(0xdeadcafe));
        let layout = client.generate_nebula_layout(&player, &region, &region, &seed);
        for i in 0..layout.size {
            let a = layout.anomalies.get(i).unwrap();
            match a.anomaly_type {
                AnomalyType::DustCloud        => saw_dust    = true,
                AnomalyType::IonStorm         => saw_ion     = true,
                AnomalyType::CrystalFormation => saw_crystal = true,
                AnomalyType::PlasmaVent       => saw_plasma  = true,
                AnomalyType::DarkMatterPocket => saw_dark    = true,
            }
        }
        advance_ledgers(&env, 3);
    }

    assert!(saw_dust,    "DustCloud never appeared");
    assert!(saw_ion,     "IonStorm never appeared");
    assert!(saw_crystal, "CrystalFormation never appeared");
    assert!(saw_plasma,  "PlasmaVent never appeared");
    assert!(saw_dark,    "DarkMatterPocket never appeared");
}

#[test]
fn test_resource_class_distribution_includes_all_tiers() {
    let (env, cid, _) = setup_env();
    let client = NebulaGenClient::new(&env, &cid);
    let player = Address::generate(&env);

    let mut saw_sparse   = false;
    let mut saw_moderate = false;
    let mut saw_abundant = false;

    for region in 0u64..30 {
        let seed = seed_from_u64(&env, region.wrapping_add(1));
        let layout = client.generate_nebula_layout(&player, &region, &region, &seed);
        for i in 0..layout.size {
            let a = layout.anomalies.get(i).unwrap();
            match a.resource_class {
                ResourceClass::Sparse   => saw_sparse   = true,
                ResourceClass::Moderate => saw_moderate = true,
                ResourceClass::Abundant => saw_abundant = true,
            }
        }
        advance_ledgers(&env, 2);
    }

    assert!(saw_sparse,   "Sparse tier never appeared");
    assert!(saw_moderate, "Moderate tier never appeared");
    assert!(saw_abundant, "Abundant tier never appeared");
}

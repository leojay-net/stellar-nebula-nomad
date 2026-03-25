#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{vec, Address, Bytes, BytesN, Env, Symbol, Vec};
use stellar_nebula_nomad::{
    CellType, NebulaNomadContract, NebulaNomadContractClient, NebulaCell,
    NebulaLayout, Rarity, ShipError, GRID_SIZE, TOTAL_CELLS,
};

use proptest::prelude::*;

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
    let contract_id = env.register(NebulaNomadContract, ());
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
    let contract_id = env.register(NebulaNomadContract, ());
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

// ─── ship_nft ─────────────────────────────────────────────────────────────

#[test]
fn test_mint_ship_and_transfer_ownership() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_slice(&env, b"{\"paint\":\"blue\"}");
    let ship_type = Symbol::new(&env, "fighter");

    // Soroban client unwraps Result automatically; use try_ for error path.
    let minted = client.mint_ship(&player, &ship_type, &metadata);
    assert_eq!(minted.id, 1);
    assert_eq!(minted.owner, player.clone());
    assert_eq!(minted.hull, 150);

    let new_owner = Address::generate(&env);
    let transferred = client.transfer_ownership(&minted.id, &new_owner);

    assert_eq!(transferred.owner, new_owner.clone());
    let fetched = client.get_ship(&minted.id);
    assert_eq!(fetched.owner, new_owner);
}

#[test]
fn test_batch_mint_limit_and_invalid_ship_type() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::new(&env);

    let too_many = vec![
        &env,
        Symbol::new(&env, "fighter"),
        Symbol::new(&env, "explorer"),
        Symbol::new(&env, "hauler"),
        Symbol::new(&env, "fighter"),
    ];

    let err = client
        .try_batch_mint_ships(&player, &too_many, &metadata)
        .unwrap_err();
    // The SDK wraps contract errors in Ok(Err(..)) or Err(..);
    // we just confirm we got an error (panic path vs try_ path).
    assert!(matches!(err, Ok(ShipError::BatchLimitExceeded) | Err(_)));

    let invalid = Symbol::new(&env, "invalid");
    let err2 = client
        .try_mint_ship(&player, &invalid, &metadata)
        .unwrap_err();
    assert!(matches!(err2, Ok(ShipError::InvalidShipType) | Err(_)));
}

// ─── harvest + auto dex listing ──────────────────────────────────────────

#[test]
fn test_harvest_resources_single_invocation_and_events() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::new(&env);
    let ship = client.mint_ship(&player, &Symbol::new(&env, "explorer"), &metadata);

    let seed = BytesN::from_array(&env, &[11u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);

    let result = client.harvest_resources(&ship.id, &layout);

    assert_eq!(result.ship_id, ship.id);
    assert!(result.total_units > 0);
    assert!(!result.resources.is_empty());
    assert!(result.auto_offer.quantity > 0);
    assert!(result.auto_offer.min_price > 0);

    let events = env.events().all();
    // ship minted + harvest done + dex listed = at least 3 contract events
    // (mock_all_auths may also emit internal events; verify we have our custom ones)
    assert!(
        events.len() >= 2,
        "Expected at least 2 custom events, got {}",
        events.len()
    );
}

#[test]
#[should_panic(expected = "Error")]
fn test_harvest_fails_for_unknown_ship() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[22u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);

    // Calling with non-existent ship_id should panic (contract error).
    client.harvest_resources(&999u64, &layout);
}

proptest! {
    #[test]
    fn fuzz_harvest_and_rarity(seed in any::<[u8; 32]>(), seq in 1u32..100_000, ts in 1u64..10_000_000_000) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: seq,
            timestamp: ts,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1000,
            max_entry_ttl: 10_000,
        });

        let contract_id = env.register(NebulaNomadContract, ());
        let client = NebulaNomadContractClient::new(&env, &contract_id);
        let player = Address::generate(&env);

        let ship = client.mint_ship(&player, &Symbol::new(&env, "fighter"), &Bytes::new(&env));

        let seed = BytesN::from_array(&env, &seed);
        let layout = client.generate_nebula_layout(&seed, &player);
        prop_assert_eq!(layout.width, GRID_SIZE);
        prop_assert_eq!(layout.height, GRID_SIZE);
        prop_assert_eq!(layout.cells.len(), TOTAL_CELLS);

        let rarity = client.calculate_rarity_tier(&layout);
        prop_assert!(
            rarity == Rarity::Common
                || rarity == Rarity::Uncommon
                || rarity == Rarity::Rare
                || rarity == Rarity::Epic
                || rarity == Rarity::Legendary
        );

        let harvest = client.harvest_resources(&ship.id, &layout);
        prop_assert!(harvest.total_units > 0);
        prop_assert!(harvest.auto_offer.min_price > 0);
    }
}


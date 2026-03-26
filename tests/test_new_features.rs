#![cfg(test)]

use soroban_sdk::{testutils::{Address as _, Events}, Address, BytesN, Env, IntoVal, String, Symbol, symbol_short};
use crate::{NebulaNomadContract, NebulaNomadContractClient};

#[test]
fn test_yield_farming_flow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    
    // Deposit
    let amount = 1000;
    let lock_period = 30 * 86400; // 30 days
    let pool_id = client.deposit_to_pool(&user, &amount, &lock_period);
    
    assert_eq!(pool_id, 0);

    // Harvest (immediately, should be 0 or small)
    let reward = client.harvest_farm_rewards(&user, &pool_id);
    assert_eq!(reward, 0);

    // Simulate time passing: 1 year
    env.ledger().set_timestamp(31_536_000);
    
    let reward_after_year = client.harvest_farm_rewards(&user, &pool_id);
    // Base APY is 15%. 1000 * 0.15 = 150.
    assert_eq!(reward_after_year, 150);
}

#[test]
fn test_governance_voting() {
    let env = Env::default();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let desc = String::from_str(&env, "Decrease fee to 1%");
    let param = BytesN::from_array(&env, &[0u8; 128]);

    let proposal_id = client.create_proposal(&creator, &desc, &param);
    assert_eq!(proposal_id, 0);

    let voter = Address::generate(&env);
    client.cast_vote(&voter, &proposal_id, &true, &50000);

    // Check events
    let events = env.events().all();
    let last_event = events.last().unwrap();
    assert_eq!(last_event.2, (proposal_id, voter, true, 50000i128).into_val(&env));
}

#[test]
fn test_theme_customizer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let theme_id = symbol_short!("nebula1");
    
    client.apply_theme(&owner, &1, &theme_id);

    // Preview
    let preview = client.generate_theme_preview(&theme_id);
    assert_eq!(preview.name, symbol_short!("Cosmic"));
}

#[test]
fn test_indexer_callbacks() {
    let env = Env::default();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let cb_id = symbol_short!("stat_bot");
    
    client.register_indexer_callback(&admin, &cb_id);
    
    let payload = BytesN::from_array(&env, &[1u8; 256]);
    client.trigger_indexer_event(&symbol_short!("alert"), &payload);
}

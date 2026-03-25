use soroban_sdk::{contracttype, symbol_short, Address, Env};

/// ── Storage Keys ──────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Auto-incrementing bond counter.
    BondCounter,
    /// Bond metadata keyed by bond_id.
    Bond(u64),
    /// Yield delegation config keyed by bond_id.
    YieldDel(u64),
    /// Cosmic essence balance for a player address.
    Essence(Address),
}

/// ── Bond Status ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum BondStatus {
    /// Bond created, waiting for partner to accept.
    Pending,
    /// Both parties confirmed — bond is live.
    Active,
    /// Bond dissolved by one of the parties.
    Dissolved,
}

/// ── Nomad Bond ────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub struct NomadBond {
    pub bond_id: u64,
    pub initiator: Address,
    pub partner: Address,
    pub ship_id: u64,
    pub status: BondStatus,
    pub created_at: u64,
}

/// ── Yield Delegation ──────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub struct YieldDelegation {
    pub bond_id: u64,
    pub delegator: Address,
    pub beneficiary: Address,
    pub percentage: u32,
    pub total_yielded: u64,
}

/// ── Helper: next bond id ──────────────────────────────────────────────────

fn next_bond_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&DataKey::BondCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&DataKey::BondCounter, &next);
    next
}

/// ── create_bond ───────────────────────────────────────────────────────────
///
/// Creates a new Nomad Bond between the caller (`initiator`) and a
/// `partner`.  The bond starts in `Pending` status until the partner
/// accepts it via `accept_bond`.
///
/// # Arguments
/// * `initiator` – The player who proposes the bond (must authorize).
/// * `ship_id`   – The ship NFT the bond is attached to.
/// * `partner`   – The address invited to bond.
///
/// # Panics
/// * If `initiator` and `partner` are the same address.
pub fn create_bond(env: &Env, initiator: &Address, ship_id: u64, partner: &Address) -> NomadBond {
    initiator.require_auth();

    if initiator == partner {
        panic!("cannot bond with yourself");
    }

    let bond_id = next_bond_id(env);
    let bond = NomadBond {
        bond_id,
        initiator: initiator.clone(),
        partner: partner.clone(),
        ship_id,
        status: BondStatus::Pending,
        created_at: env.ledger().timestamp(),
    };

    env.storage().instance().set(&DataKey::Bond(bond_id), &bond);

    env.events().publish(
        (symbol_short!("bond"), symbol_short!("created")),
        (bond_id, initiator.clone(), partner.clone()),
    );

    bond
}

/// ── accept_bond ───────────────────────────────────────────────────────────
///
/// The invited partner accepts a pending bond, moving it to `Active`.
///
/// # Panics
/// * If bond does not exist.
/// * If caller is not the designated partner.
/// * If bond is not in `Pending` status.
pub fn accept_bond(env: &Env, partner: &Address, bond_id: u64) -> NomadBond {
    partner.require_auth();

    let mut bond: NomadBond = env
        .storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .expect("bond not found");

    if bond.partner != *partner {
        panic!("only the designated partner can accept");
    }
    if bond.status != BondStatus::Pending {
        panic!("bond is not pending");
    }

    bond.status = BondStatus::Active;
    env.storage().instance().set(&DataKey::Bond(bond_id), &bond);

    env.events().publish(
        (symbol_short!("bond"), symbol_short!("accepted")),
        (bond_id, partner.clone()),
    );

    bond
}

/// ── delegate_yield ────────────────────────────────────────────────────────
///
/// Allows one bonded party (the `delegator`) to share a percentage of
/// their accrued cosmic essence with the other bonded party.
///
/// # Arguments
/// * `delegator`  – Must be either `initiator` or `partner` of the bond.
/// * `bond_id`    – An active bond.
/// * `percentage` – 1–100 inclusive.
///
/// # Panics
/// * If bond does not exist or is not `Active`.
/// * If the caller is not part of the bond.
/// * If `percentage` is out of range.
pub fn delegate_yield(
    env: &Env,
    delegator: &Address,
    bond_id: u64,
    percentage: u32,
) -> YieldDelegation {
    delegator.require_auth();

    if percentage == 0 || percentage > 100 {
        panic!("percentage must be 1-100");
    }

    let bond: NomadBond = env
        .storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .expect("bond not found");

    if bond.status != BondStatus::Active {
        panic!("bond is not active");
    }

    let beneficiary = if *delegator == bond.initiator {
        bond.partner.clone()
    } else if *delegator == bond.partner {
        bond.initiator.clone()
    } else {
        panic!("caller is not part of this bond");
    };

    let delegation = YieldDelegation {
        bond_id,
        delegator: delegator.clone(),
        beneficiary: beneficiary.clone(),
        percentage,
        total_yielded: 0,
    };

    env.storage()
        .instance()
        .set(&DataKey::YieldDel(bond_id), &delegation);

    env.events().publish(
        (symbol_short!("yield"), symbol_short!("delegatd")),
        (bond_id, delegator.clone(), percentage),
    );

    delegation
}

/// ── accrue_essence ────────────────────────────────────────────────────────
///
/// Award cosmic essence to a player.  Called by game logic (e.g. after a
/// successful nebula scan).  In a production setup this would be an
/// internal / cross-contract call from the `NebulaExplorer` contract.
pub fn accrue_essence(env: &Env, player: &Address, amount: u64) {
    let balance: u64 = env
        .storage()
        .instance()
        .get(&DataKey::Essence(player.clone()))
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::Essence(player.clone()), &(balance + amount));
}

/// ── claim_yield ───────────────────────────────────────────────────────────
///
/// The beneficiary of a yield delegation claims their share.  The
/// delegator's cosmic essence is reduced by the delegated percentage and
/// transferred to the beneficiary.
///
/// # Security
/// * Only the beneficiary address recorded in the delegation can claim.
/// * The bond must still be `Active`.
/// * If the delegator has zero balance, nothing is transferred.
///
/// Returns the amount transferred.
pub fn claim_yield(env: &Env, claimer: &Address, bond_id: u64) -> u64 {
    claimer.require_auth();

    let bond: NomadBond = env
        .storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .expect("bond not found");

    if bond.status != BondStatus::Active {
        panic!("bond is not active");
    }

    let mut delegation: YieldDelegation = env
        .storage()
        .instance()
        .get(&DataKey::YieldDel(bond_id))
        .expect("no yield delegation for this bond");

    if *claimer != delegation.beneficiary {
        panic!("only the beneficiary can claim yield");
    }

    let delegator_balance: u64 = env
        .storage()
        .instance()
        .get(&DataKey::Essence(delegation.delegator.clone()))
        .unwrap_or(0);

    if delegator_balance == 0 {
        return 0;
    }

    let yield_amount = (delegator_balance * delegation.percentage as u64) / 100;

    if yield_amount == 0 {
        return 0;
    }

    // Debit delegator
    env.storage().instance().set(
        &DataKey::Essence(delegation.delegator.clone()),
        &(delegator_balance - yield_amount),
    );

    // Credit beneficiary
    let claimer_balance: u64 = env
        .storage()
        .instance()
        .get(&DataKey::Essence(claimer.clone()))
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::Essence(claimer.clone()), &(claimer_balance + yield_amount));

    delegation.total_yielded += yield_amount;
    env.storage()
        .instance()
        .set(&DataKey::YieldDel(bond_id), &delegation);

    env.events().publish(
        (symbol_short!("yield"), symbol_short!("claimed")),
        (bond_id, claimer.clone(), yield_amount),
    );

    yield_amount
}

/// ── dissolve_bond ─────────────────────────────────────────────────────────
///
/// Either the initiator or partner can dissolve an active bond.
/// Once dissolved, no further yield claims can be made.
pub fn dissolve_bond(env: &Env, caller: &Address, bond_id: u64) -> NomadBond {
    caller.require_auth();

    let mut bond: NomadBond = env
        .storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .expect("bond not found");

    if bond.status == BondStatus::Dissolved {
        panic!("bond is already dissolved");
    }

    if *caller != bond.initiator && *caller != bond.partner {
        panic!("only bonded parties can dissolve");
    }

    bond.status = BondStatus::Dissolved;
    env.storage().instance().set(&DataKey::Bond(bond_id), &bond);

    env.events().publish(
        (symbol_short!("bond"), symbol_short!("dissolve")),
        (bond_id, caller.clone()),
    );

    bond
}

/// ── get_bond ──────────────────────────────────────────────────────────────
///
/// Read-only view of a bond by its ID.
pub fn get_bond(env: &Env, bond_id: u64) -> NomadBond {
    env.storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .expect("bond not found")
}

/// ── get_yield_delegation ──────────────────────────────────────────────────
///
/// Read-only view of a yield delegation by its bond ID.
pub fn get_yield_delegation(env: &Env, bond_id: u64) -> YieldDelegation {
    env.storage()
        .instance()
        .get(&DataKey::YieldDel(bond_id))
        .expect("no yield delegation for this bond")
}

/// ── get_essence_balance ───────────────────────────────────────────────────
///
/// Read-only view of a player's cosmic essence balance.
pub fn get_essence_balance(env: &Env, player: &Address) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::Essence(player.clone()))
        .unwrap_or(0)
}

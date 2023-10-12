#![no_std]

use core::{panic};

use soroban_sdk::{
    contractimpl, contracttype, symbol, Address, BytesN, ConversionError,
    Env, RawVal, Symbol, TryFromVal, Vec,
};

mod token {
    soroban_sdk::contractimport!(file = "./soroban_token_spec.wasm");
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SecretVote {
    pub voter: Address,
    pub prop_id: u32,
}

#[derive(Clone)]
#[contracttype]
pub enum HiddenData {
    PrivateAdmin,
    TotalSupply,
    UserBalance(Address),
    SecretBootstrap,
    EncryptedProposal(u32),
    CurrentProposalId,
    VoteRecord(SecretVote),
    ExecutionStatus(u32)
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct HiddenInstruction {
    //contract id
    pub contract_id: BytesN<32>,
    pub function_name: Symbol,
    pub arguments: Vec<RawVal>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ClassifiedProposal {
    pub total_votes: i32,
    pub deadline: u64,
    // instructions will be executed in sequence
    pub instructions: Vec<HiddenInstruction>,
}

pub trait UndisclosedDaoTrait {
    fn initialize(env: Env);
    fn transfer_hidden_assets(env: Env, amount: i32, to: Address);
    fn create_secret_proposal(env: Env, proposal: ClassifiedProposal) -> u32;
    fn attempt_execution(env: Env, prop_id: u32);
    fn check_user_assets(env: Env, owner: Address) -> i32;
    fn total_concealed_assets(env: Env) -> i32;
    fn cast_secret_ballot(env: Env, prop_id: u32);
}

pub struct SecretDaoContract;

#[contractimpl]
impl UndisclosedDaoTrait for SecretDaoContract {
    fn initialize(env: Env) {
        if None != verify_secret_admin(&env) {
            panic!();
        }
        // Designate the caller as the confidential administrator
        set_secret_administrator(&env, env.invoker());
        // Allocate 1 concealed asset to the caller
        transfer_hidden_assets(&env, 1, env.invoker());

        // Allow the administrator to disburse concealed assets for a week
        env.data()
            .set(HiddenData::SecretBootstrap, env.ledger().timestamp() + 3600 * 24 * 7);
    }

    fn transfer_hidden_assets(env: Env, amount: i32, to: Address) {
        // Trigger an error if not the administrator
        if !validate_secret_admin(&env) {
            panic!();
        }

        // Trigger an error if not within the bootstrap period
        if !check_secret_bootstrap_period(&env) {
            panic!()
        }
        allocate_hidden_assets(&env, amount, to)
    }

    fn create_secret_proposal(env: Env, proposal: ClassifiedProposal) -> u32 {
        assert!(proposal.total_votes == 0);

        let next_id = acquire_and_increment_secret_proposal_id(&env);

        env.data().set(HiddenData::EncryptedProposal(next_id), proposal.clone());

        next_id
    }

    fn attempt_execution(env: Env, prop_id: u32) {

        // Execution is only permitted once
        assert!(!has_been_executed(&env, prop_id));

        let prop = env
            .data()
            .get::<_, ClassifiedProposal>(HiddenData::EncryptedProposal(prop_id))
            .unwrap()
            .unwrap();

        // Execution can only take place prior to the deadline
        assert!(prop.deadline > env.ledger().timestamp());
        // Majority of concealed assets is required for execution
        assert!(prop.total_votes > total_concealed_assets(&env) / 2);

        // Code block that doesn't work
        // let authorized_contract_functions = map![&env, (symbol!("authorized_concealed_assets_fn"), Self::authorized_concealed_assets_fn)];

        for result in prop.instructions {
            match result {
                Ok(instruction) => {
                    if env.current_contract() == instruction.contract_id {
                        if instruction.function_name == symbol!("transfer_hidden_assets") {
                            let amount =
                                i32::try_from_val(&env, instruction.arguments.get(0).unwrap().unwrap())
                                    .unwrap();
                            let recipient =
                                Address::try_from_val(&env, instruction.arguments.get(1).unwrap().unwrap())
                                    .unwrap();
                            allocate_hidden_assets(&env, amount, recipient);
                        }
                    } else {
                        env.invoke_contract(&instruction.contract_id, &instruction.function_name, instruction.arguments)
                    }
                }
                Err(_) => panic!(),
            }
        }
        record_execution(&env, prop_id);
    }

    fn check_user_assets(env: Env, owner: Address) -> i32 {
        access_hidden_assets(&env, owner)
    }

    fn total_concealed_assets(env: Env) -> i32 {
        compute_total_hidden_assets(&env)
    }

    fn cast_secret_ballot(env: Env, prop_id: u32) {
        assert!(!has_cast_vote(&env, env.invoker(), prop_id));

        let mut prop = env
            .data()
            .get::<_, ClassifiedProposal>(HiddenData::EncryptedProposal(prop_id))
            .unwrap()
            .unwrap();

        // Verify the validity of the proposal
        assert!(prop.deadline > env.ledger().timestamp());

        let user_assets = access_hidden_assets(&env, env.invoker());

        prop.total_votes = prop.total_votes + user_assets;

        env.data()
        .set(HiddenData::EncryptedProposal(prop_id), prop);

        env.data().set(
            HiddenData::VoteRecord(SecretVote {
                voter: env.invoker(),
                prop_id,
            }),
            true,
        );
    }
}

fn record_execution(env: &Env, prop_id: u32){
    env.data().set(HiddenData::ExecutionStatus(prop_id), true)
}

fn has_been_executed(env: &Env, prop_id: u32)-> bool{
    env.data().get(HiddenData::ExecutionStatus(prop_id))
    .unwrap_or(Ok(false))
    .unwrap()
}

fn has_cast_vote(env: &Env, voter: Address, prop_id: u32) -> bool{
    return env.data().get(HiddenData::VoteRecord(SecretVote {
        voter: voter.clone(),
        prop_id,
    })).unwrap_or(Ok(false)).unwrap()
}

fn acquire_and_increment_secret_proposal_id(env: &Env) -> u32 {
    let previous = env
        .data()
        .get(HiddenData::CurrentProposalId)
        .unwrap_or(Ok(0u32))
        .unwrap();

    env.data().set(HiddenData::CurrentProposalId, previous + 1);
    previous
}

fn check_secret_bootstrap_period(env: &Env) -> bool {
    env.data()
        .get::<_, u64>(HiddenData::SecretBootstrap)
        .unwrap()
        .unwrap()
        > env.ledger().timestamp()
}

fn access_hidden_assets(env: &Env, owner: Address) -> i32 {
    env.data()
        .get(HiddenData::UserBalance(owner))
        .unwrap_or(Ok(0))
        .unwrap()
}

fn allocate_hidden_assets(env: &Env, amount: i32, to: Address) {
    let current_assets = env
        .data()
        .get(HiddenData::UserBalance(to.clone()))
        .unwrap_or(Ok(0))
        .unwrap();

    env.data()
        .set(HiddenData::UserBalance(to), amount + current_assets);

    modify_total_hidden_assets(env, amount)
}

fn modify_total_hidden_assets(env: &Env, amount: i32) {
    let total_assets = compute_total_hidden_assets(env);

    env.data().set(HiddenData::TotalSupply, total_assets + amount)
}

fn compute_total_hidden_assets(env: &Env) -> i32 {
    let total_assets = env.data().get(HiddenData::TotalSupply).unwrap_or(Ok(0)).unwrap();
    total_assets
}

fn validate_secret_admin(env: &Env) -> bool {
    env.invoker() == env.data().get(HiddenData::PrivateAdmin).unwrap().unwrap()
}

fn verify_secret_admin(env: &Env) -> Option<Result<Address, ConversionError>> {
    env.data().get(HiddenData::PrivateAdmin)
}

fn set_secret_administrator(env: &Env, admin: Address) {
    env.data().set(HiddenData::PrivateAdmin, admin)
}

#[cfg(test)]
mod disguised_test;

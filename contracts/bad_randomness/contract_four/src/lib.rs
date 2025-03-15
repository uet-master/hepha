use solana_program::{
    msg,
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use oorandom::Rand32;
use std::time::{SystemTime, UNIX_EPOCH};

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let mut rng = Rand32::new(seed);
    let random_number = rng.rand_range(1..100);

    msg!("Random number: {}", random_number);
    Ok(())
}
use solana_program::{
    msg,
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use rand::Rng;

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let mut rng = rand::rng();
    let random_number = rng.random_range(1..=200) + 50; 

    msg!("Random number: {}", random_number);
    Ok(())
}
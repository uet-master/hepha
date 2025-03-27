use solana_program::{
    msg,
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use nanorand::{Rng, WyRand};

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let mut rng = WyRand::new();
    let random_number = rng.generate_range(1..=150) + 200;

    msg!("Random number: {}", random_number);
    Ok(())
}
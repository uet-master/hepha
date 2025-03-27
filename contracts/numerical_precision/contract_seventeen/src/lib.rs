use solana_program::{
    msg,
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar}
};

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let user_account = next_account_info(accounts_iter)?;

    let mut values = vec![15.0, 20.0, 35.0, 40.0, 65.0];

    if !user_account.is_signer {
        msg!("User account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let clock = Clock::get()?;
    let block_timestamp = clock.unix_timestamp as f64;
    let random_number = (block_timestamp / (60 as f64)).round();

    values[2] = values[3] * random_number;
    Ok(())
}

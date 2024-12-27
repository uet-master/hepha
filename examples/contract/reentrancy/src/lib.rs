use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let user_account = next_account_info(account_info_iter)?;

    if user_account.is_signer {
        msg!("Reentrant call detected!");
        return Err(ProgramError::Custom(0));
    }

    let balance_before = user_account.lamports();
    msg!("User balance before: {}", balance_before);

    let amount_to_withdraw = 1_000;
    **user_account.try_borrow_mut_lamports()? -= amount_to_withdraw;

    msg!("Attempting reentrant call...");

    let balance_after = user_account.lamports();
    msg!("User balance after: {}", balance_after);

    Ok(())
}
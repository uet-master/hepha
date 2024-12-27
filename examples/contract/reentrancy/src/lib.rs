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
    let contract_account = next_account_info(account_info_iter)?;

    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let withdraw_amount = u64::from_le_bytes(instruction_data[0..9].try_into().unwrap());

    if **contract_account.try_borrow_lamports()? < withdraw_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    **contract_account.try_borrow_mut_lamports()? -= withdraw_amount;
    **user_account.try_borrow_mut_lamports()? += withdraw_amount;

    msg!("Withdrawal successful: {} lamports", withdraw_amount);
    Ok(())
}
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
    let state_account = next_account_info(account_info_iter)?;

    if !state_account.is_writable || state_account.owner != program_id {
        return Err(ProgramError::InvalidAccountData);
    }

    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let instruction = instruction_data[0];
    match instruction {
        0 => deposit(user_account, contract_account, state_account, instruction_data),
        1 => withdraw(user_account, contract_account, state_account, instruction_data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn deposit(
    user_account: &AccountInfo,
    contract_account: &AccountInfo,
    state_account: &AccountInfo,
    instruction_data: &[u8]
) -> ProgramResult {
    let deposit_amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());

    if **user_account.try_borrow_lamports()? < deposit_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    **user_account.try_borrow_mut_lamports()? -= deposit_amount;
    **contract_account.try_borrow_mut_lamports()? += deposit_amount;

    // Update the new balance for the 
    let mut state_data = state_account.try_borrow_mut_data()?;
    let total_deposited = u64::from_le_bytes(state_data[0..8].try_into().unwrap());
    let updated_total = total_deposited + deposit_amount;
    state_data[0..8].copy_from_slice(&updated_total.to_le_bytes());

    msg!("Deposit successful: {} lamports", deposit_amount);
    msg!("Total deposited: {} lamports", updated_total);
    Ok(())
}

fn withdraw(
    user_account: &AccountInfo,
    contract_account: &AccountInfo,
    state_account: &AccountInfo,
    instruction_data: &[u8]
) -> ProgramResult {
    if instruction_data.len() < 9 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let withdraw_amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());

    if **contract_account.try_borrow_lamports()? < withdraw_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    **contract_account.try_borrow_mut_lamports()? -= withdraw_amount;
    **user_account.try_borrow_mut_lamports()? += withdraw_amount;

    let mut state_data = state_account.try_borrow_mut_data()?;
    let total_withdrawn = u64::from_le_bytes(state_data[8..16].try_into().unwrap());
    let updated_total = total_withdrawn + withdraw_amount;
    state_data[8..16].copy_from_slice(&updated_total.to_le_bytes());

    msg!("Withdrawal successful: {} lamports", withdraw_amount);
    msg!("Total withdrawn: {} lamports", updated_total);
    Ok(())
}

use solana_program::{
    msg,
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let user_account = next_account_info(accounts_iter)?;
    let contract_account = next_account_info(accounts_iter)?;

    let mut contract_balance = 0;

    if !user_account.is_signer {
        msg!("User account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let instruction = instruction_data[0];
    match instruction {
        0 => {
            let amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());
            deposit(&mut contract_balance, amount, user_account, contract_account)?;
        }
        1 => {
            let input_number = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());
            announce_winner(&mut contract_balance, input_number, user_account, contract_account)?;
        }
        _ => {
            msg!("Invalid action");
            return Err(ProgramError::InvalidInstructionData);
        }
    }

    Ok(())
}

pub fn deposit(
    contract_balance: &mut u64, 
    amount: u64,
    user_account: &AccountInfo,
    contract_account: &AccountInfo
) -> Result<(), ProgramError>  {
    **user_account.try_borrow_mut_lamports()? -= amount;
    **contract_account.try_borrow_mut_lamports()? += amount;
    *contract_balance += amount;
    Ok(())
}

pub fn announce_winner(
    contract_balance: &mut u64,
    input_number: u64,
    user_account: &AccountInfo,
    contract_account: &AccountInfo
) -> Result<(), ProgramError> {
    let random_number = fastrand::u64(1..1000000);

    if input_number == random_number {
        **contract_account.try_borrow_mut_lamports()? -= 0;
        **user_account.try_borrow_mut_lamports()? += *contract_balance;
        *contract_balance = 0;
    }

    Ok(())
}
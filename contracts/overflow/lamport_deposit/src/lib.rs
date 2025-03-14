use solana_program::{
    msg,
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::collections::HashMap;

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let user_account = next_account_info(accounts_iter)?;
    let contract_account = next_account_info(accounts_iter)?;

    let mut balances: HashMap<Pubkey, u64> = HashMap::new();

    if !user_account.is_signer {
        msg!("User account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let instruction = instruction_data[0];
    let amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());
    match instruction {
        0 => {
            msg!("User deposits {} lamports", amount);
            deposit(&mut balances, *user_account.key, amount, user_account, contract_account)?;
        }
        _ => {
            msg!("Invalid action");
            return Err(ProgramError::InvalidInstructionData);
        }
    }

    Ok(())
}

pub fn deposit(
    balances: &mut HashMap<Pubkey, u64>, 
    user: Pubkey, 
    amount: u64,
    user_account: &AccountInfo,
    contract_account: &AccountInfo
) -> Result<(), ProgramError>  {
    let entry = balances.entry(user).or_insert(0);
    *entry += amount;
    
    **user_account.try_borrow_mut_lamports()? -= amount;
    **contract_account.try_borrow_mut_lamports()? += amount;
    Ok(())
}


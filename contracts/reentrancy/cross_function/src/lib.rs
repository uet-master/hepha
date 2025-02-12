use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
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
    // If the user calls the deposit function, the external_account is the receiver.
    // If the user calls the other functions, the external_account is the contract_account.
    let external_account = next_account_info(accounts_iter)?;

    let mut balances: HashMap<Pubkey, u64> = HashMap::new();

    if !user_account.is_signer {
        msg!("User account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let instruction = instruction_data[0];

    match instruction {
        0 => {
            let amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());
            msg!("User deposits {} lamports", amount);
            deposit_lamports(&mut balances, *user_account.key, amount, user_account, external_account)?;
        }
        1 => {
            let amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());
            msg!("User transfers {} lamports", amount);
            transfer_lamports(&mut balances, *user_account.key, *external_account.key, amount)?;
        }
        2 => {
            msg!("User withdraws all lamports");
            withdraw_all(&mut balances, user_account.key, user_account, external_account)?;
        }
        _ => {
            msg!("Invalid action");
            return Err(ProgramError::InvalidInstructionData);
        }
    }

    let balance = get_balance(&mut balances, user_account.key);
    msg!(
        "User {} has a remaining balance of {} lamports",
        user_account.key,
        balance
    );

    Ok(())
}

pub fn deposit_lamports(
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

pub fn transfer_lamports(
    balances: &mut HashMap<Pubkey, u64>, 
    user: Pubkey, 
    receiver: Pubkey,
    amount: u64
) -> Result<(), ProgramError> {
    let user_balance = balances.get(&user).ok_or(ProgramError::InvalidAccountData).copied()?;
    let receiver_balance = balances.get(&receiver).ok_or(ProgramError::InvalidAccountData).copied()?;

    if user_balance < amount {
        msg!("Insufficient balance for transfer");
        return Err(ProgramError::InsufficientFunds);
    }

    balances.insert(user, user_balance - amount);
    balances.insert(receiver, receiver_balance - amount);
    Ok(())
}

pub fn withdraw_all(
    balances: &mut HashMap<Pubkey, u64>,  
    user: &Pubkey, 
    user_account: &AccountInfo,
    contract_account: &AccountInfo
) -> Result<(), ProgramError> {
    let balance = balances.get_mut(user).ok_or(ProgramError::InvalidAccountData)?;
    **contract_account.try_borrow_mut_lamports()? -= *balance;
    **user_account.try_borrow_mut_lamports()? += *balance;

    *balance = 0;
    Ok(())
}

pub fn get_balance(balances: &mut HashMap<Pubkey, u64>, user: &Pubkey) -> u64 {
    *balances.get(user).unwrap_or(&0)
}
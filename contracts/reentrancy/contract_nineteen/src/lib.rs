use solana_program::{
    msg,
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_system_interface::instruction;
use std::collections::HashMap;

entrypoint!(process_instruction);

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum Instruction {
    Deposit,
    WithdrawAll
}

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let user_account = next_account_info(accounts_iter)?;

    let mut balances: HashMap<Pubkey, u64> = HashMap::new();
    let x = instruction_data[0];
    let instruction = Instruction::try_from_slice(&[instruction_data[0]])?;
    match instruction {
        Instruction::Deposit => {
            let amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());
            msg!("User deposits {} lamports", amount);
            deposit(&mut balances, accounts, amount)?;
        }
        Instruction::WithdrawAll => {
            withdraw_all(&mut balances, accounts)?;
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

pub fn deposit(balances: &mut HashMap<Pubkey, u64>, accounts: &[AccountInfo], amount: u64) -> Result<(), ProgramError>  {
    let accounts_iter = &mut accounts.iter();
    let user_account = next_account_info(accounts_iter)?;
    let contract_account = next_account_info(accounts_iter)?;

    if !user_account.is_signer {
        msg!("User account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let entry = balances.entry(*user_account.key).or_insert(0);
    *entry += amount;

    instruction::transfer(user_account.key, contract_account.key, amount);
    Ok(())
}

pub fn withdraw_all(balances: &mut HashMap<Pubkey, u64>, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
    let accounts_iter = &mut accounts.iter();
    let user_account = next_account_info(accounts_iter)?;
    let contract_account = next_account_info(accounts_iter)?;

    if !contract_account.is_signer {
        msg!("Contract account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let balance = balances.get_mut(user_account.key).ok_or(ProgramError::InvalidAccountData)?;
    instruction::transfer(contract_account.key, user_account.key, *balance);

    *balance = 0;
    Ok(())
}

pub fn get_balance(balances: &mut HashMap<Pubkey, u64>, user: &Pubkey) -> u64 {
    *balances.get(user).unwrap_or(&0)
}
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::collections::HashMap;

#[derive(Default)]
pub struct DepositContract {
    pub deposits: HashMap<Pubkey, u64>, // Key: User account, Value: Deposited tokens
}

impl DepositContract {
    pub fn deposit(&mut self, user: Pubkey, amount: u64) {
        let entry = self.deposits.entry(user).or_insert(0);
        *entry += amount;
    }

    pub fn withdraw(&mut self, user: &Pubkey, amount: u64) -> Result<(), ProgramError> {
        let balance = self.deposits.get_mut(user).ok_or(ProgramError::InvalidAccountData)?;
        if *balance < amount {
            msg!("Insufficient balance for withdrawal");
            return Err(ProgramError::InsufficientFunds);
        }
        *balance -= amount;
        Ok(())
    }

    pub fn get_balance(&self, user: &Pubkey) -> u64 {
        *self.deposits.get(user).unwrap_or(&0)
    }
}

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let user_account = next_account_info(accounts_iter)?;
    let contract_account = next_account_info(accounts_iter)?;

    if !user_account.is_signer {
        msg!("User account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let instruction = instruction_data[0];
    let amount = u64::from_le_bytes(instruction_data[1..9].try_into().unwrap());
    let mut deposit_contract = DepositContract::default();

    match instruction {
        0 => {
            msg!("User deposits {} lamports", amount);
            deposit_contract.deposit(*user_account.key, amount);
            **user_account.try_borrow_mut_lamports()? -= amount;
            **contract_account.try_borrow_mut_lamports()? += amount;
        }
        1 => {
            msg!("User withdraws {} lamports", amount);
            **contract_account.try_borrow_mut_lamports()? -= amount;
            **user_account.try_borrow_mut_lamports()? += amount;
            deposit_contract.withdraw(user_account.key, amount)?;
        }
        _ => {
            msg!("Invalid action");
            return Err(ProgramError::InvalidInstructionData);
        }
    }

    let balance = deposit_contract.get_balance(user_account.key);
    msg!(
        "User {} has a remaining balance of {} lamports",
        user_account.key,
        balance
    );

    Ok(())
}
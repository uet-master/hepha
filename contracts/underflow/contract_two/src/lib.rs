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

    let mut values: HashMap<Pubkey, u64> = HashMap::new();

    if !user_account.is_signer {
        msg!("User account must sign the transaction");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let instruction = instruction_data[0];
    let data = user_account.try_borrow_mut_data()?;
    let sample_number = u64::from_le_bytes(data[..8].try_into().unwrap());
    match instruction {
        0 => {
            substract(&mut values, *user_account.key, sample_number)?;
        }
        _ => {
            msg!("Invalid action");
            return Err(ProgramError::InvalidInstructionData);
        }
    }

    Ok(())
}

pub fn substract(
    values: &mut HashMap<Pubkey, u64>, 
    user: Pubkey, 
    sample_number: u64,
) -> Result<(), ProgramError>  {
    let entry = values.entry(user).or_insert(0);
    *entry -= sample_number * 50;
    
    Ok(())
}
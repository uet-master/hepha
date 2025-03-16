use solana_program::{
    msg,
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::collections::HashMap;
use oorandom::Rand64;
use std::time::{SystemTime, UNIX_EPOCH};

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
    match instruction {
        0 => {
            substract(&mut values, *user_account.key)?;
        }
        _ => {
            msg!("Invalid action");
            return Err(ProgramError::InvalidInstructionData);
        }
    }

    Ok(())
}

pub fn substract(values: &mut HashMap<Pubkey, u64>, user: Pubkey) -> Result<(), ProgramError>  {
    let entry = values.entry(user).or_insert(0);
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let mut rng = Rand64::new(seed.into());
    let random_number = rng.rand_range(1..100);
    *entry -= random_number;
    
    Ok(())
}
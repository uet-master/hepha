use anchor_lang::prelude::*;

declare_id!("5XxEBr4oRQ3L8ejXTEqtt3jUsRNZSgBYLfCSocQ6BtKv");

#[program]
pub mod attack {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

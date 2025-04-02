use anchor_lang::prelude::*;
use std::mem::size_of;

declare_id!("4YxE5GRA7UsNwLtpyQcL3F245F6te4Gg2BPAhMvWoKh5");

#[program]
pub mod solana_fee_predictor {
    use super::*;

    // Initialize fee data account
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    // Store fee data
    pub fn store_fee_data(ctx: Context<StoreFeeData>, fee: u64, tps: u32) -> Result<()> {
        let fee_data = &mut ctx.accounts.fee_data;
        
        // Keep only the last 144 samples (~24 hours if stored every 10 mins)
        if fee_data.fees.len() >= 144 {
            fee_data.fees.remove(0);
            fee_data.timestamps.remove(0);
            fee_data.tps_values.remove(0);
        }
        
        fee_data.fees.push(fee);
        fee_data.timestamps.push(Clock::get()?.unix_timestamp);
        fee_data.tps_values.push(tps);
        
        Ok(())
    }

    // Predict best time to transact
    pub fn predict_best_time(ctx: Context<PredictBestTime>) -> Result<u64> {
        let fee_data = &ctx.accounts.fee_data;
        
        if fee_data.fees.is_empty() {
            return Ok(0); // Default fee if no data
        }
        
        // Simple prediction: return the minimum fee
        Ok(*fee_data.fees.iter().min().unwrap_or(&0))
    }
}

// Data account storing historical fees
#[account]
pub struct FeeData {
    pub fees: Vec<u64>,       // Priority fees
    pub timestamps: Vec<i64>,  // Unix timestamps
    pub tps_values: Vec<u32>, // Transactions per second
}

// Context for initialization
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + // Discriminator
                (4 + 144 * 8) + // fees: Vec<u64> (max 144 entries)
                (4 + 144 * 8) + // timestamps: Vec<i64>
                (4 + 144 * 4),  // tps_values: Vec<u32>
        seeds = [b"fee_data", payer.key().as_ref()],
        bump
    )]
    pub fee_data: Account<'info, FeeData>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Context for storing fee data
#[derive(Accounts)]
pub struct StoreFeeData<'info> {
    #[account(mut)]
    pub fee_data: Account<'info, FeeData>,
    pub payer: Signer<'info>,
}

// Context for predicting best time
#[derive(Accounts)]
pub struct PredictBestTime<'info> {
    pub fee_data: Account<'info, FeeData>,
}

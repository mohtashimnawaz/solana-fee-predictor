use anchor_lang::prelude::*;

declare_id!("4YxE5GRA7UsNwLtpyQcL3F245F6te4Gg2BPAhMvWoKh5");

#[program]
pub mod solana_fee_predictor {
    use super::*;

    /// Initialize fee data account
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let fee_data = &mut ctx.accounts.fee_data;
        fee_data.authority = *ctx.accounts.payer.key;
        fee_data.last_updated = Clock::get()?.unix_timestamp;
        msg!("Fee data account initialized");
        Ok(())
    }

    /// Store fee data with network metrics
    pub fn store_fee_data(
        ctx: Context<StoreFeeData>,
        fee: u64,
        tps: u32,
        slot: u64,
        compute_units_consumed: u64,
    ) -> Result<()> {
        let fee_data = &mut ctx.accounts.fee_data;
        
        // Maintain rolling window of 144 samples (~24 hours if stored every 10 mins)
        if fee_data.historical_data.len() >= 144 {
            fee_data.historical_data.remove(0);
        }
        
        fee_data.historical_data.push(FeeSample {
            fee,
            tps,
            slot,
            compute_units_consumed: compute_units_consumed,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        fee_data.last_updated = Clock::get()?.unix_timestamp;
        msg!("Stored new fee data at slot {}", slot);
        
        Ok(())
    }

    /// Predict optimal fee based on historical data
    pub fn predict_fee(
        ctx: Context<PredictFee>,
        compute_units_estimate: u64,
        priority_level: PriorityLevel,
    ) -> Result<FeePrediction> {
        let fee_data = &ctx.accounts.fee_data;
        
        if fee_data.historical_data.is_empty() {
            msg!("No historical data available - returning default prediction");
            return Ok(FeePrediction::default());
        }
        
        // Calculate statistics
        let avg_fee = calculate_average(&fee_data.historical_data, |s| s.fee);
        let avg_compute_units = calculate_average(&fee_data.historical_data, |s| s.compute_units_consumed);
        
        // Adjust for priority level
        let priority_multiplier = priority_level.multiplier();
        
        // Scale fee based on compute units
        let compute_scaling = if avg_compute_units > 0 {
            compute_units_estimate as f64 / avg_compute_units as f64
        } else {
            1.0
        };
        
        let estimated_fee = (avg_fee as f64 * priority_multiplier * compute_scaling) as u64;
        
        Ok(FeePrediction {
            estimated_fee,
            last_updated: fee_data.last_updated,
            confidence: calculate_confidence(&fee_data.historical_data),
            priority_level,
        })
    }
}

/// Fee data account structure
#[account]
pub struct FeeData {
    pub authority: Pubkey,
    pub last_updated: i64,
    pub historical_data: Vec<FeeSample>,
}

/// Individual fee sample
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FeeSample {
    pub fee: u64,
    pub tps: u32,
    pub slot: u64,
    pub compute_units_consumed: u64,
    pub timestamp: i64,
}

/// Priority level for transactions
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum PriorityLevel {
    Low,
    Medium,
    High,
}

impl PriorityLevel {
    pub fn multiplier(&self) -> f64 {
        match self {
            PriorityLevel::Low => 0.8,
            PriorityLevel::Medium => 1.0,
            PriorityLevel::High => 1.5,
        }
    }
}

impl Default for PriorityLevel {
    fn default() -> Self {
        PriorityLevel::Medium
    }
}

/// Fee prediction result
#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub struct FeePrediction {
    pub estimated_fee: u64,
    pub last_updated: i64,
    pub confidence: u8, // 0-100
    pub priority_level: PriorityLevel,
}

/// Initialize context
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 8 + (4 + 144 * std::mem::size_of::<FeeSample>()), // ~12KB
        seeds = [b"fee_data", payer.key().as_ref()],
        bump
    )]
    pub fee_data: Account<'info, FeeData>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Store fee data context
#[derive(Accounts)]
pub struct StoreFeeData<'info> {
    #[account(
        mut,
        has_one = authority @ ErrorCode::Unauthorized
    )]
    pub fee_data: Account<'info, FeeData>,
    pub authority: Signer<'info>,
}

/// Predict fee context
#[derive(Accounts)]
pub struct PredictFee<'info> {
    #[account()]
    pub fee_data: Account<'info, FeeData>,
}

/// Helper function to calculate average of a field
fn calculate_average<F>(data: &[FeeSample], field: F) -> u64 
where
    F: Fn(&FeeSample) -> u64,
{
    if data.is_empty() {
        return 0;
    }
    data.iter().map(field).sum::<u64>() / data.len() as u64
}

/// Calculate confidence score (0-100)
fn calculate_confidence(data: &[FeeSample]) -> u8 {
    if data.len() < 2 {
        return 0;
    }
    
    let avg = data.iter().map(|s| s.fee).sum::<u64>() as f64 / data.len() as f64;
    let variance = data.iter()
        .map(|s| (s.fee as f64 - avg).powi(2))
        .sum::<f64>() / data.len() as f64;
    
    // Higher variance = lower confidence
    (100.0 / (1.0 + variance.sqrt())).min(100.0) as u8
}

/// Error codes
#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Insufficient historical data")]
    InsufficientData,
}
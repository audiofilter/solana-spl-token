#![feature(trivial_bounds)]

use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, TokenAccount, Transfer, Token},
};
use anchor_lang::solana_program::instruction::Instruction;

declare_id!("AeDRnzQh9VJswa71LWQrcP1R1AhT2bdtKmkjsCvyUzq6");

/// Constants for tax distribution
pub const TAX_PERCENTAGE: u64 = 10; // 10% tax
pub const REWARDS_POOL_SHARE: u64 = 40; // 40% of tax
pub const LIQUIDITY_GROWTH_SHARE: u64 = 30; // 30% of tax
pub const TEAM_FEE_SHARE: u64 = 10; // 10% of tax
pub const TREASURY_RESERVE_SHARE: u64 = 10; // 10% of tax
pub const MARKETING_SHARE: u64 = 5; // 5% of tax
pub const DEVELOPMENT_SHARE: u64 = 5; // 5% of tax

#[program]
pub mod solami_rewards {
    use super::*;

    /// Initialize the treasury account
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Initializing treasury account");
        ctx.accounts.treasury_account.taxed_amount = 0;
        ctx.accounts.treasury_account.total_rewards_distributed = 0;
        Ok(())
    }

    /// Transfer with tax distribution
    pub fn transfer_with_tax(ctx: Context<TransferWithTax>, amount: u64) -> Result<()> {
        require!(amount > 0, CustomError::InvalidAmount);
        msg!("Transferring with {}% tax", TAX_PERCENTAGE);
        
        let tax = amount.checked_mul(TAX_PERCENTAGE)
            .ok_or(CustomError::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(CustomError::ArithmeticOverflow)?;
        
        let net_amount = amount.checked_sub(tax)
            .ok_or(CustomError::ArithmeticOverflow)?;

        // Transfer net amount to recipient
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.sender_token.to_account_info(),
                    to: ctx.accounts.recipient_token.to_account_info(),
                    authority: ctx.accounts.sender.to_account_info(),
                },
            ),
            net_amount,
        )?;

        // Calculate tax distributions
        let tax_distributions = calculate_tax_distributions(tax)?;

        // Transfer tax portions to respective accounts
        for (amount, destination) in tax_distributions {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.sender_token.to_account_info(),
                        to: destination.to_account_info(),
                        authority: ctx.accounts.sender.to_account_info(),
                    },
                ),
                amount,
            )?;
        }

        // Update treasury state
        ctx.accounts.treasury_account.taxed_amount = ctx.accounts.treasury_account.taxed_amount
            .checked_add(tax)
            .ok_or(CustomError::ArithmeticOverflow)?;
        
        Ok(())
    }

    /// Swap taxed tokens for SOL via Jupiter
    pub fn swap_taxed_tokens(ctx: Context<SwapTaxedTokens>, swap_instructions: Vec<Instruction>) -> Result<()> {
        require!(!swap_instructions.is_empty(), CustomError::EmptySwapInstructions);
        msg!("Executing swap instructions");
        
        for instruction in swap_instructions {
            anchor_lang::solana_program::program::invoke(
                &instruction,
                &ctx.accounts.to_account_infos(),
            )?;
        }

        ctx.accounts.treasury_account.taxed_amount = 0;
        Ok(())
    }

    /// Claim user rewards
    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        msg!("Claiming rewards");
        let user_rewards = ctx.accounts.user_rewards.pending_rewards;
        require!(user_rewards > 0, CustomError::NoRewards);

        // Transfer rewards
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? = ctx.accounts.user
            .to_account_info()
            .lamports()
            .checked_add(user_rewards)
            .ok_or(CustomError::ArithmeticOverflow)?;
        
        ctx.accounts.user_rewards.pending_rewards = 0;
        ctx.accounts.treasury_account.total_rewards_distributed = ctx.accounts.treasury_account
            .total_rewards_distributed
            .checked_add(user_rewards)
            .ok_or(CustomError::ArithmeticOverflow)?;

        Ok(())
    }
}

/// Calculate tax distributions for different pools
fn calculate_tax_distributions(tax: u64) -> Result<Vec<(u64, AccountInfo)>> {
    let mut distributions = Vec::new();
    
    let rewards_pool = tax.checked_mul(REWARDS_POOL_SHARE)
        .ok_or(CustomError::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(CustomError::ArithmeticOverflow)?;
    
    let liquidity_growth = tax.checked_mul(LIQUIDITY_GROWTH_SHARE)
        .ok_or(CustomError::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(CustomError::ArithmeticOverflow)?;
    
    // Add other distributions similarly...
    
    Ok(distributions)
}

/// **Accounts**
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + TreasuryAccount::LEN,
        seeds = [b"treasury"],
        bump
    )]
    pub treasury_account: Account<'info, TreasuryAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TransferWithTax<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    #[account(
        mut,
        constraint = sender_token.owner == sender.key(),
        constraint = sender_token.mint == treasury_token.mint
    )]
    pub sender_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub recipient_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub treasury_token: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"treasury"],
        bump
    )]
    pub treasury_account: Account<'info, TreasuryAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct SwapTaxedTokens<'info> {
    #[account(
        mut,
        seeds = [b"treasury"],
        bump
    )]
    pub treasury_account: Account<'info, TreasuryAccount>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        constraint = user_rewards.owner == user.key()
    )]
    pub user_rewards: Account<'info, UserRewards>,
    #[account(
        mut,
        seeds = [b"treasury"],
        bump
    )]
    pub treasury_account: Account<'info, TreasuryAccount>,
}

#[account]
pub struct TreasuryAccount {
    pub taxed_amount: u64,
    pub total_rewards_distributed: u64,
}

impl TreasuryAccount {
    pub const LEN: usize = 8 + 8 + 8; // discriminator + taxed_amount + total_rewards_distributed
}

#[account]
pub struct UserRewards {
    pub owner: Pubkey,
    pub pending_rewards: u64,
}

#[error_code]
pub enum CustomError {
    #[msg("No rewards to claim")]
    NoRewards,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Empty swap instructions")]
    EmptySwapInstructions,
}

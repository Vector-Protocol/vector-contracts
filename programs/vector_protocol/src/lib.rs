//! Vector Protocol: Final Version for Anchor 0.30.0+

use anchor_lang::prelude::*;
// CORRECTED `use` statements:
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod vector_protocol {
    use super::*;

    pub fn create_pool(ctx: Context<CreatePool>, trade_fee_bps: u16) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.token_a_mint = ctx.accounts.token_a_mint.key();
        pool.token_b_mint = ctx.accounts.token_b_mint.key();
        pool.token_a_vault = ctx.accounts.token_a_vault.key();
        pool.token_b_vault = ctx.accounts.token_b_vault.key();
        pool.lp_mint = ctx.accounts.lp_mint.key();
        pool.trade_fee_bps = trade_fee_bps;
        pool.bump = ctx.bumps.pool;
        Ok(())
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount_a: u64, amount_b: u64) -> Result<()> {
        let pool = &ctx.accounts.pool;
        let lp_mint = &ctx.accounts.lp_mint;
        let lp_to_mint = if lp_mint.supply == 0 {
            1_000_000_000
        } else {
            (lp_mint.supply * amount_a) / ctx.accounts.token_a_vault.amount
        };

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_a.to_account_info(),
                    to: ctx.accounts.token_a_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_a,
        )?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_b.to_account_info(),
                    to: ctx.accounts.token_b_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_b,
        )?;

        let pool_seeds = &[
            b"pool".as_ref(),
            pool.token_a_mint.as_ref(),
            pool.token_b_mint.as_ref(),
            &[pool.bump],
        ];
        let signer = &[&pool_seeds[..]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    to: ctx.accounts.user_lp_wallet.to_account_info(),
                    authority: pool.to_account_info(),
                },
                signer,
            ),
            lp_to_mint,
        )?;
        Ok(())
    }

    pub fn swap(ctx: Context<Swap>, amount_in: u64) -> Result<()> {
        let vault_source_balance = ctx.accounts.vault_source.amount;
        let vault_destination_balance = ctx.accounts.vault_destination.amount;
        let fee = (amount_in * ctx.accounts.pool.trade_fee_bps as u64) / 10000;
        let amount_in_after_fee = amount_in.checked_sub(fee).ok_or(PoolError::CalculationError)?;
        let amount_out = vault_destination_balance
            .checked_mul(amount_in_after_fee)
            .ok_or(PoolError::CalculationError)?
            .checked_div(vault_source_balance.checked_add(amount_in_after_fee).ok_or(PoolError::CalculationError)?)
            .ok_or(PoolError::CalculationError)?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_source.to_account_info(),
                    to: ctx.accounts.vault_source.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_in,
        )?;

        let pool = &ctx.accounts.pool;
        let pool_seeds = &[
            b"pool".as_ref(),
            pool.token_a_mint.as_ref(),
            pool.token_b_mint.as_ref(),
            &[pool.bump],
        ];
        let signer = &[&pool_seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_destination.to_account_info(),
                    to: ctx.accounts.user_destination.to_account_info(),
                    authority: pool.to_account_info(),
                },
                signer,
            ),
            amount_out,
        )?;
        Ok(())
    }
}

#[account]
#[derive(Default)]
pub struct Pool {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub trade_fee_bps: u16,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 32 + 32 + 32 + 32 + 2 + 1,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,
    #[account(constraint = token_a_mint.key() != token_b_mint.key())]
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    #[account(
        constraint = token_a_vault.owner == pool.key(),
        constraint = token_a_vault.mint == token_a_mint.key()
    )]
    pub token_a_vault: Account<'info, TokenAccount>,
    #[account(
        constraint = token_b_vault.owner == pool.key(),
        constraint = token_b_vault.mint == token_b_mint.key()
    )]
    pub token_b_vault: Account<'info, TokenAccount>,
    #[account(
        constraint = lp_mint.mint_authority.unwrap() == pool.key(),
        constraint = lp_mint.supply == 0
    )]
    pub lp_mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token_a: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_b: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_lp_wallet: Account<'info, TokenAccount>,
    #[account(mut, has_one = token_a_mint, has_one = token_b_mint)]
    pub pool: Account<'info, Pool>,
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    #[account(mut, constraint = token_a_vault.key() == pool.token_a_vault)]
    pub token_a_vault: Account<'info, TokenAccount>,
    #[account(mut, constraint = token_b_vault.key() == pool.token_b_vault)]
    pub token_b_vault: Account<'info, TokenAccount>,
    #[account(mut, constraint = lp_mint.key() == pool.lp_mint)]
    pub lp_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    pub user: Signer<'info>,
    #[account(has_one = token_a_mint, has_one = token_b_mint)]
    pub pool: Account<'info, Pool>,
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_source: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_destination: Account<'info, TokenAccount>,
    #[account(mut, constraint = vault_source.key() == pool.token_a_vault || vault_source.key() == pool.token_b_vault)]
    pub vault_source: Account<'info, TokenAccount>,
    #[account(mut, constraint = vault_destination.key() == pool.token_a_vault || vault_destination.key() == pool.token_b_vault)]
    pub vault_destination: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[error_code]
pub enum PoolError {
    #[msg("Error in calculation.")]
    CalculationError,
}
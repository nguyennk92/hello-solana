use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
};
use spl_token::state::Account as TokenAccount;

use crate::{entrypoint::EscrowInstruction, error::EscrowError, state::Escrow};

pub struct Processor;
impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = EscrowInstruction::unpack(instruction_data)?;

        match instruction {
            EscrowInstruction::InitEscrow { amount } => {
                msg!("Instruction: InitEscrow");
                return Self::process_init_escrow(accounts, amount, program_id);
            }
            EscrowInstruction::Exchange { amount } => {
                msg!("Instruction: Exchange");
                return Self::process_exchange(accounts, amount, program_id);
            }
        }
    }

    /// Accounts expected:
    ///
    /// 0. `[signer]` The account of the person initializing the escrow
    /// 1. `[writable]` Temporary token account that should be created prior to this instruction and owned by the initializer
    /// 2. `[]` The initializer's token account for the token they will receive should the trade go through
    /// 3. `[writable]` The escrow account, it will hold all necessary info about the trade.
    /// 4. `[]` The rent sysvar
    /// 5. `[]` The token program
    fn process_init_escrow(
        accounts: &[AccountInfo],
        amount: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let initializer = next_account_info(account_info_iter)?;

        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let temp_token_account = next_account_info(account_info_iter)?;
        let token_to_receive_account = next_account_info(account_info_iter)?;
        if *token_to_receive_account.owner != spl_token::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        let escrow_account = next_account_info(account_info_iter)?;
        let rent_account = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(rent_account)?;

        if !rent.is_exempt(escrow_account.lamports(), escrow_account.data_len()) {
            return Err(EscrowError::NotRentExempt.into());
        }

        let mut escrow_info = Escrow::unpack_unchecked(&escrow_account.try_borrow_data()?)?;
        escrow_info.is_initialized = true;
        escrow_info.initializer_pubkey = *initializer.key;
        escrow_info.temp_token_account_pubkey = *temp_token_account.key;
        escrow_info.initializer_token_to_receive_account_pubkey = *token_to_receive_account.key;
        escrow_info.expected_ammount = amount;

        Escrow::pack(escrow_info, &mut escrow_account.try_borrow_mut_data()?)?;

        let (pda, _bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);
        let token_program = next_account_info(account_info_iter)?;
        let change_token_owner_ix = spl_token::instruction::set_authority(
            token_program.key,
            temp_token_account.key,
            Some(&pda),
            spl_token::instruction::AuthorityType::AccountOwner,
            initializer.key,
            &[initializer.key],
        )?;
        invoke(
            &change_token_owner_ix,
            &[
                temp_token_account.clone(),
                initializer.clone(),
                token_program.clone(),
            ],
        )?;

        return Ok(());
    }

    /// Accounts expected:
    ///
    /// 0. `[signer]` The account of the person taking the trade
    /// 1. `[writable]` The taker's token account for the token they send
    /// 2. `[writable]` The taker's token account for the token they will receive should the trade go through
    /// 3. `[writable]` The PDA's temp token account to get tokens from and eventually close
    /// 4. `[writable]` The initializer's main account to send their rent fees to
    /// 5. `[writable]` The initializer's token account that will receive tokens
    /// 6. `[writable]` The escrow account holding the escrow info
    /// 7. `[]` The token program
    /// 8. `[]` The PDA account
    fn process_exchange(
        accounts: &[AccountInfo],
        amount: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let signer = next_account_info(account_info_iter)?;
        if !signer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let taker_send_token_account = next_account_info(account_info_iter)?;
        let taker_receive_token_account = next_account_info(account_info_iter)?;
        let temp_token_account = next_account_info(account_info_iter)?;
        let initializer_main_account = next_account_info(account_info_iter)?;
        let initializer_to_receive_account = next_account_info(account_info_iter)?;
        let escrow_account = next_account_info(account_info_iter)?;
        let token_program = next_account_info(account_info_iter)?;
        let pda_account = next_account_info(account_info_iter)?;

        let temp_token_account_info = TokenAccount::unpack(&temp_token_account.try_borrow_data()?)?;
        if temp_token_account_info.amount != amount {
            return Err(EscrowError::InvalidAmount.into());
        }

        let escrow_info = Escrow::unpack(&escrow_account.try_borrow_data()?)?;
        if escrow_info.initializer_pubkey != *initializer_main_account.key {
            return Err(ProgramError::InvalidAccountData);
        }
        if escrow_info.initializer_token_to_receive_account_pubkey
            != *initializer_to_receive_account.key
        {
            return Err(ProgramError::InvalidAccountData);
        }

        let (pda, bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);
        let transfer_to_initializer_ix = spl_token::instruction::transfer(
            token_program.key,
            taker_send_token_account.key,
            initializer_to_receive_account.key,
            signer.key,
            &[&signer.key],
            escrow_info.expected_ammount,
        )?;
        msg!("calling transfer to initalizer");
        invoke(
            &transfer_to_initializer_ix,
            &[
                taker_send_token_account.clone(),
                initializer_to_receive_account.clone(),
                signer.clone(),
                token_program.clone(),
            ],
        )?;
        let transfer_to_taker_ix = spl_token::instruction::transfer(
            token_program.key,
            temp_token_account.key,
            taker_receive_token_account.key,
            &pda,
            &[&pda],
            amount,
        )?;
        msg!("calling transfer to taker");
        invoke_signed(
            &transfer_to_taker_ix,
            &[
                temp_token_account.clone(),
                taker_receive_token_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;
        let close_temp_token_account_ix = spl_token::instruction::close_account(
            token_program.key,
            temp_token_account.key,
            initializer_main_account.key,
            &pda,
            &[&pda],
        )?;
        msg!("calling close temp account");
        invoke_signed(
            &close_temp_token_account_ix,
            &[
                temp_token_account.clone(),
                initializer_main_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;
        msg!("closing escrow_account");
        **initializer_main_account.try_borrow_mut_lamports()? = initializer_main_account
            .lamports()
            .checked_add(escrow_account.lamports())
            .ok_or(EscrowError::AmountOverflow)?;
        **escrow_account.try_borrow_mut_lamports()? = 0;
        *escrow_account.try_borrow_mut_data()? = &mut [];

        return Ok(());
    }
}

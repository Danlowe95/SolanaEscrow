use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    msg,
    pubkey::Pubkey,
    program_pack::{Pack, IsInitialized},
    sysvar::{rent::Rent, Sysvar},
    program::invoke,
    program::invoke_signed,
};
use spl_token::state::Account as TokenAccount;
use crate::{instruction::EscrowInstruction, error::EscrowError, state::Escrow};

pub struct Processor;
impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
        let instruction = EscrowInstruction::unpack(instruction_data)?;

        match instruction {
            EscrowInstruction::InitEscrow { amount } => {
                msg!("Instruction: InitEscrow");
                Self::process_init_escrow(accounts, amount, program_id)
            },
            EscrowInstruction::Exchange { amount } => {
                msg! ("Instruction: Exchange");
                Self::process_exchange(accounts, amount, program_id)
            }
        }
    }

    fn process_exchange(
        accounts: &[AccountInfo],
        amount: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let taker = next_account_info(account_info_iter)?;
        
        if !taker.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        // token account for token Y
        let taker_sent_token_account = next_account_info(account_info_iter)?;

        // token account for token X
        let taker_receive_token_account = next_account_info(account_info_iter)?;
        let pda_temp_token_account = next_account_info(account_info_iter)?;
        let initializer_main_account = next_account_info(account_info_iter)?;
        let initializer_receive_token_account = next_account_info(account_info_iter)?;
        let escrow_account = next_account_info(account_info_iter)?;
        let token_program = next_account_info(account_info_iter)?;
        let pda_account = next_account_info(account_info_iter)?;
       

        let escrow_info = Escrow::unpack_unchecked(&escrow_account.try_borrow_data()?)?;
        if !escrow_info.is_initialized() {
            return Err(EscrowError::AccountNotInitialized.into());
        }
        if escrow_info.initializer_token_to_receive_account_pubkey != *initializer_receive_token_account.key {
            return Err(ProgramError::InvalidAccountData);
        }
        if escrow_info.initializer_pubkey != *initializer_main_account.key {
            return Err(ProgramError::InvalidAccountData);
        }
        if escrow_info.temp_token_account_pubkey != *pda_temp_token_account.key {
            return Err(ProgramError::InvalidAccountData);
        }
    
        // Check taker has sent the expected amount of token Y -- edit, do not need to check, transaction will fail if they do not.
        let pda_temp_token_account_info = TokenAccount::unpack(&pda_temp_token_account.try_borrow_data()?)?;
        // Check taker Y against token Y owner ID?
        // Check taker X against token X owner ID?
        // Check rent again (against what?)?
        
        if amount != pda_temp_token_account_info.amount {
            return Err(EscrowError::ExpectedAmountMismatch.into());
        }
        let (pda, bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);

        // What is token_program? Is it a shared token program of all tokens?
        // Write instructions for token transfers
        let transfer_to_initializer_ix = spl_token::instruction::transfer(
            token_program.key,
            taker_sent_token_account.key,
            initializer_receive_token_account.key,
            taker.key,
            &[&taker.key],
            escrow_info.expected_amount
        )?;
        // invoke_signed token transfers
        msg!("Calling the token program to transfer tokens to the escrow's initializer...");
        invoke(
            &transfer_to_initializer_ix,
            &[
                taker_sent_token_account.clone(),
                initializer_receive_token_account.clone(),
                taker.clone(),
                token_program.clone(),
            ]
        )?;
        let transfer_to_taker_ix = spl_token::instruction::transfer(
            token_program.key,
            pda_temp_token_account.key,
            taker_receive_token_account.key,
            &pda,
            &[&pda],
            pda_temp_token_account_info.amount
        )?;
        msg!("Calling the token program to transfer tokens to the taker...");

        invoke_signed(
            &transfer_to_taker_ix,
            &[
                pda_temp_token_account.clone(),
                taker_receive_token_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;
        // Cleanup floating accounts

        let close_pdas_temp_acc_ix = spl_token::instruction::close_account(
            token_program.key,
            pda_temp_token_account.key,
            initializer_main_account.key,
            &pda,
            &[&pda]
        )?;
        msg!("Calling the token program to close pda's temp account...");
        invoke_signed(
            &close_pdas_temp_acc_ix,
            &[
                pda_temp_token_account.clone(),
                initializer_main_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;
        
        msg!("Closing the escrow account...");
        **initializer_main_account.lamports.borrow_mut() = initializer_main_account.lamports()
        .checked_add(escrow_account.lamports())
        .ok_or(EscrowError::AmountOverflow)?;
        **escrow_account.lamports.borrow_mut() = 0;
        *escrow_account.try_borrow_mut_data()? = &mut [];

        Ok(())
    }

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
        
        // TODO how does this actually work
        // Is this just checking the owner _is_ a token ID?
        // What if I wanted to check if the owner is a specific Token Mint Address?
        if *token_to_receive_account.owner != spl_token::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        let escrow_account = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

        if !rent.is_exempt(escrow_account.lamports(), escrow_account.data_len()) {
            return Err(EscrowError::NotRentExempt.into());
        }

        let mut escrow_info = Escrow::unpack_unchecked(&escrow_account.try_borrow_data()?)?;
        if escrow_info.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        escrow_info.is_initialized = true;
        escrow_info.initializer_pubkey = *initializer.key;
        escrow_info.temp_token_account_pubkey = *temp_token_account.key;
        escrow_info.initializer_token_to_receive_account_pubkey = *token_to_receive_account.key;
        escrow_info.expected_amount = amount;

        Escrow::pack(escrow_info, &mut escrow_account.try_borrow_mut_data()?)?;
        
        let (pda, _bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);
        
        let token_program = next_account_info(account_info_iter)?;
        let owner_change_ix = spl_token::instruction::set_authority(
            token_program.key,
            temp_token_account.key,
            // TODO Why does Some need to be used here?
            Some(&pda),
            spl_token::instruction::AuthorityType::AccountOwner,
            initializer.key,
            &[&initializer.key],
        )?;
        
        msg!("Calling the token program to transfer token account ownership...");
        invoke(
            &owner_change_ix,
            &[
                temp_token_account.clone(),
                initializer.clone(),
                token_program.clone(),
            ],
        )?;

        Ok(())
    }
}
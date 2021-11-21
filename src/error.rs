// inside error.rs
use thiserror::Error;
use solana_program::program_error::ProgramError;

#[derive(Error, Debug, Copy, Clone)]
pub enum EscrowError {
    /// Invalid instruction
    #[error("Invalid Instruction")]
    InvalidInstruction,

    /// Not rent exempt
    #[error("Escrow account is not rent exempt")]
    NotRentExempt,

    #[error("Escrow account is not initialized")]
    AccountNotInitialized,
    
    #[error("Invalid instruction data")]
    InvalidInstructionData,

    #[error("Invalid instruction data")]
    ExpectedAmountMismatch,
    #[error("Amount overflow")]
    AmountOverflow
}

impl From<EscrowError> for ProgramError {
    fn from(e: EscrowError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
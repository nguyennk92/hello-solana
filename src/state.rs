use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

pub struct Escrow {
    pub is_initialized: bool,
    pub initializer_pubkey: Pubkey,
    pub temp_token_account_pubkey: Pubkey,
    pub initializer_token_to_receive_account_pubkey: Pubkey,
    pub expected_ammount: u64,
}

impl Sealed for Escrow {}

impl IsInitialized for Escrow {
    fn is_initialized(&self) -> bool {
        return self.is_initialized;
    }
}

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

impl Pack for Escrow {
    const LEN: usize = 1 + 32 + 32 + 32 + 8;
    fn unpack_from_slice(src: &[u8]) -> Result<Self, solana_program::program_error::ProgramError> {
        let src = array_ref![src, 0, Escrow::LEN];
        let (
            is_initialized,
            initializer_pubkey,
            temp_token_account_pubkey,
            initializer_token_to_receive_account_pubkey,
            expected_amount,
        ) = array_refs![src, 1, 32, 32, 32, 8];
        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        return Ok(Escrow {
            is_initialized,
            initializer_pubkey: Pubkey::new_from_array(*initializer_pubkey),
            temp_token_account_pubkey: Pubkey::new_from_array(*temp_token_account_pubkey),
            initializer_token_to_receive_account_pubkey: Pubkey::new_from_array(
                *initializer_token_to_receive_account_pubkey,
            ),
            expected_ammount: u64::from_le_bytes(*expected_amount),
        });
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref!(dst, 0, Escrow::LEN);
        let (
            is_initialized_dst,
            initializer_pubkey_dst,
            temp_token_account_pubkey_dst,
            initializer_token_to_receive_account_pubkey_dst,
            expected_amount_dst,
        ) = mut_array_refs!(dst, 1, 32, 32, 32, 8);

        is_initialized_dst[0] = self.is_initialized as u8;
        initializer_pubkey_dst.copy_from_slice(self.initializer_pubkey.as_ref());
        temp_token_account_pubkey_dst.copy_from_slice(self.temp_token_account_pubkey.as_ref());
        initializer_token_to_receive_account_pubkey_dst
            .copy_from_slice(self.initializer_token_to_receive_account_pubkey.as_ref());
        *expected_amount_dst = u64::to_le_bytes(self.expected_ammount);
    }
}

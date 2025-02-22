//! # DID Offchain Trade Pallet
//!
//! The DID Offchain Trade pallet allows trading data access control rights at offchain.
//!
//! ## Overview
//! 
//! The DID Offchain Trade pallet provides functionality for data access control rights trading.
//!
//! * Create Access Condition
//! * Update on-chain condition by co-singed state proof.
//! * Set New DID
//!
//! ### Terminology
//!
//! * **DID** A Decentralized Identifiers/Identity compliant with the DID standard.
//!		The DID is an AccountId with associated attributes/properties.
//! * **Access Condition** Access Condition allows managing and resolving payment logic.
//! * **DocumentPermissionsState** DocumentPermissionsState manage who has data access control rights.
//! 
//! ### Goals
//! The DID Offchain Trade system in designed to make the following possible:
//!
//! * Users control their data. 
//! * Manage data access control rights transparently. 
//! * Trading data access control rights without trusted third party.
//!
//! ### Dispatchable Functions
//!
//! * `create_access_condition` - Create a new Access Condition from channel peer.
//! * `intend_settle` - Update Access Condition and DocumentPermissionsState by co-signed state proof from channel peer.
//! * `get_access_condition` - Get field of Access Condition.
//!
//!	### Dispatchable Functions
//!
//! * `is_finalized` - Returns a boolen value. `True` if the AppStatus is FINALIZED. AppStatus is field of AccessCondition.
//! * `get_outcome` - Returns a boolean value. `True` if the outcome which is field of AccessCondition is true. 
//! * `check_permissions` - Returns a boolean value. `True` if the grantee has data access control rights.
//! * `get_nonce` - Get the nonce which is field of AccessCondition.
//! * `get_seq_num` - Get the sequence number which is field of AccessCondition.
//! * `get_status` - Get the AppStatus which is field of AccessCondition. AppStatus is IDLE or FINALIZED.
//! * `get_owner` - Get the owner which is field of AccessCondition.
//! * `get_grantee` - Get the grantee which is field of AccessCondition.
//!
//! ## Dependencies
//!
//! This pallet depends on the DID pallet and secret store module.
//!
//! *

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use pallet_did;
use frame_support::{
	decl_module, decl_storage, decl_event, decl_error, 
	dispatch::DispatchResult, ensure, 
	storage::{StorageMap, StorageDoubleMap},
};
use sp_runtime::traits::{IdentifyAccount, Member, Verify};
use sp_std::{prelude::*, vec::Vec};
use frame_system::{self as system, ensure_signed};
use sp_core::{RuntimeDebug};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod mock;

/// Access Condition 
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug)]
pub struct AccessCondition<AccountId> {
	pub nonce: u32,
	pub players: Vec<AccountId>,
	pub seq_num: u32,
	pub status: AppStatus,
	pub outcome: bool,
	pub owner: AccountId,
	pub grantee: AccountId,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Encode, Decode, RuntimeDebug)]
pub enum AppStatus {
	IDLE,
	FINALIZED,
}

type AccessConditionOf<T> = AccessCondition<<T as system::Trait>::AccountId>;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug)]
pub struct State<AccountId> {
	pub condition_address: AccountId,
	pub op: u8,
	pub did: Option<AccountId>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug)]
pub struct AppState<AccountId> {
	pub nonce: u32,
	pub seq_num: u32,
	pub state: State<AccountId>,
}

/// Co-signed state proof
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug)]
pub struct StateProof<Signature, AccountId> {
	pub app_state: AppState<AccountId>,
	pub sigs: Vec<Signature>,
}

/// The pallet's configuration trait.
pub trait Trait: system::Trait + pallet_did::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Public: IdentifyAccount<AccountId = Self::AccountId>;
	type Signature: Verify<Signer = <Self as Trait>::Public> + Member + Decode + Encode;
}

decl_storage! {
	trait Store for Module<T: Trait> as DIDTOffchainTrade {
		/// The set of address of Access Condition and Access Condition. 
		pub AccessConditionList get(fn condition_list): 
			map hasher(twox_64_concat) T::AccountId => Option<AccessConditionOf<T>>;
		
		/// First account is DID and second account is grantee.
		/// If grantee has data access control right, DocumentPermissionsStates is 1.
		pub DocumentPermissionsStates get(fn permission):
			double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) T::AccountId => u8;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Create Access Condition.
		pub fn create_access_condition(
			origin,
			players: Vec<T::AccountId>, 
			nonce: u32,
			did: T::AccountId,
			condition_address: T::AccountId
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			/// Checks if number of channel peer is 2.
			ensure!(players.len() == 2, Error::<T>::InvalidPlayerLength);

			let owner = match <pallet_did::Module<T>>::owner_of(&did) {
				Some(_owner) => _owner,
				None => return Err(Error::<T>::NotExist.into())
			};
			/// Check if channel peer is owner of did.
			ensure!(owner == players[0] || owner == players[1], Error::<T>::NotOwner);

			/// Check if address of Access Condition is not exist.
			ensure!(<AccessConditionList<T>>::contains_key(&condition_address) == false, Error::<T>::ExistAddress);
			
			if owner == players[0] {
				/// Add Access Condition.
				Self::set_access_condition(condition_address, nonce, players[0].clone(), players[1].clone())?;
			} else {
				/// Add Access Condition.
				Self::set_access_condition(condition_address, nonce, players[1].clone(), players[0].clone())?;
			}

			Ok(())
		}

		/// Update Access Condition and DocumentPermissionsState by co-signed state proof from channel peer.
		pub fn intend_settle(
			origin, 
			transaction: StateProof<<T as Trait>::Signature, T::AccountId>,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let condition_address = transaction.app_state.state.condition_address.clone();
			
			/// Get Access Condition.
			let access_condition = match Self::condition_list(&condition_address) {
				Some(_condtion) => _condtion,
				None => return Err(Error::<T>::InvalidConditionAddress.into())
			};

			/// Checks if a nonce is valid.
			ensure!(access_condition.nonce == transaction.app_state.nonce, Error::<T>::InvalidNonce);
			
			/// Checks if a sequence number is higher than previous one.
			ensure!(access_condition.seq_num < transaction.app_state.seq_num, Error::<T>::InvalidSeqNum);
			
			let players = vec![access_condition.owner.clone(), access_condition.grantee.clone()];
			
			if transaction.app_state.state.op == 0 {
			/// If state.op is 0, AppStatus update from FINALED to IDLE and replace owner and grantee.
			
				/// Checks if a state proof is signed by channel peer.
				let mut encoded = transaction.app_state.nonce.encode();
				encoded.extend(transaction.app_state.seq_num.encode());
				encoded.extend(transaction.app_state.state.condition_address.clone().encode());
				encoded.extend(transaction.app_state.state.op.encode());
				Self::valid_signers(transaction.sigs, &encoded, players)?;

				/// Check if AppStatus is FINALIZED.
				ensure!(access_condition.status == AppStatus::FINALIZED, Error::<T>::NotFinalizedStatus);

				let players: Vec<T::AccountId> = vec![access_condition.players[0].clone(), access_condition.players[1].clone()];
				let new_access_condition = AccessConditionOf::<T> {
					nonce: access_condition.nonce,
					players: players,
					seq_num: transaction.app_state.seq_num,
					status: AppStatus::IDLE,
					outcome: false,
					owner: access_condition.grantee.clone(),
					grantee: access_condition.owner.clone(),
				};
				
				/// Update Access Condition.
				<AccessConditionList<T>>::mutate(&condition_address, |new| *new = Some(new_access_condition.clone()));
				Self::deposit_event(
					RawEvent::SwapPosition(
						condition_address,
						<frame_system::Module<T>>::block_number(),
					)
				);
			} else if transaction.app_state.state.op == 1 {
			/// If state.op is 1, AppStatus update from FINALIZED to IDLE.
				
				/// Check if a state proof is signed by channel peer.
				let mut encoded = transaction.app_state.nonce.encode();
				encoded.extend(transaction.app_state.seq_num.encode());
				encoded.extend(transaction.app_state.state.condition_address.clone().encode());
				encoded.extend(transaction.app_state.state.op.encode());
				Self::valid_signers(transaction.sigs, &encoded, players)?;
				
				/// Check if AppStatus is FINALIZED.
				ensure!(access_condition.status == AppStatus::FINALIZED, Error::<T>::NotFinalizedStatus);
				
				let players: Vec<T::AccountId> = vec![access_condition.players[0].clone(), access_condition.players[1].clone()];
				let new_access_condition = AccessConditionOf::<T> {
					nonce: access_condition.nonce,
					players: players,
					seq_num: transaction.app_state.seq_num,
					status: AppStatus::IDLE,
					outcome: false,
					owner: access_condition.owner.clone(),
					grantee: access_condition.grantee.clone(),
				};

				/// Update Access Condition.
				<AccessConditionList<T>>::mutate(&condition_address, |new| *new = Some(new_access_condition.clone()));
				
				Self::deposit_event(
					RawEvent::SetIdle(
						condition_address,
						<frame_system::Module<T>>::block_number(),
					)
				);
			} else if transaction.app_state.state.op == 2 {
			/// If state.op is 2, grantee is granted data access control rights, 
			/// AppStatus update from IDLE to FINALIZED and outcome update true.
			
				let did = match transaction.app_state.state.did {
					Some(_did) => _did,
					None => return Err(Error::<T>::NotExist.into())
				};

				/// Checks if a state proof is signed by channel peer.
				let mut encoded = transaction.app_state.nonce.encode();
				encoded.extend(transaction.app_state.seq_num.encode());
				encoded.extend(transaction.app_state.state.condition_address.clone().encode());
				encoded.extend(transaction.app_state.state.op.encode());
				encoded.extend(did.clone().encode());
				Self::valid_signers(transaction.sigs, &encoded, players)?;

				let did_owner = match <pallet_did::Module<T>>::owner_of(&did) {
					Some(_owner) => _owner,
					None => return Err(Error::<T>::NotExist.into())
				};

				/// Check if did owner is valid.
				ensure!(&did_owner == &access_condition.owner, Error::<T>::NotOwner);
				
				/// Check if AppStatus is IDLE.
				ensure!(access_condition.status == AppStatus::IDLE, Error::<T>::NotIdleStatus);

				let new_access_condition = AccessConditionOf::<T> {
					nonce: access_condition.nonce,
					players: access_condition.players.clone(),
					seq_num: transaction.app_state.seq_num,
					status: AppStatus::FINALIZED,
					outcome: true,
					owner: access_condition.owner.clone(),
					grantee: access_condition.grantee.clone(),
				};

				/// Update Access condition.
				<AccessConditionList<T>>::mutate(&condition_address, |new| *new = Some(new_access_condition.clone()));
				
				/// Add DocumentPermissionState.
				<DocumentPermissionsStates<T>>::insert(&did, &access_condition.grantee, 1);
			
				Self::deposit_event(
					RawEvent::IntendSettle(
						condition_address,
						<frame_system::Module<T>>::block_number(),
					)
				);
			}
			
			Ok(())
		}

		/// Get Access Condition.
		pub fn get_access_condition(origin, condition_address: T::AccountId) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			
			let access_condition = match Self::condition_list(&condition_address) {
				Some(_condition) => _condition,
				None => return Err(Error::<T>::InvalidConditionAddress.into())
			};

			Self::deposit_event(
				RawEvent::AccessCondition(
					access_condition.nonce,
					access_condition.players,
					access_condition.seq_num,
					access_condition.owner,
					access_condition.grantee
				)
			);

			Ok(())
		}
	}
}

decl_event!(
	pub enum Event<T>
	where
	<T as frame_system::Trait>::AccountId,
	<T as frame_system::Trait>::BlockNumber,
	{
		AccessConditionCreated(AccountId, AccountId, AccountId),
		SwapPosition(AccountId, BlockNumber),
		SetIdle(AccountId, BlockNumber),
		IntendSettle(AccountId, BlockNumber),
		AccessCondition(u32, Vec<AccountId>, u32, AccountId, AccountId),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		NotOwner,
		InvalidPlayerLength,
		InvalidSender,
		InvalidState,
		InvalidNonce,
		InvalidSeqNum,
		InvalidSignature,
		ExistAddress,
		NotIdleStatus,
		NotFinalizedStatus,
		InvalidConditionAddress,
		NotExist,

	}
}

impl<T: Trait> Module<T> {
	/// Checks if signature is valid.
	pub fn valid_signers(
		signatures: Vec<<T as Trait>::Signature>,
		encoded: &[u8],
		signers: Vec<T::AccountId>,
	) -> DispatchResult {
		let signature1 = &signatures[0];
		let signature2 = &signatures[1];
		if signature1.verify(encoded, &signers[0]) && signature2.verify(encoded, &signers[1]) {
			Ok(())
		} else if signature1.verify(encoded, &signers[1]) && signature2.verify(encoded, &signers[0]) {
			Ok(())
		} else {
			Err(Error::<T>::InvalidSignature.into())
		}
	}

	/// Set Access Condition.
	fn set_access_condition(
		condition_address: T::AccountId, 
		nonce: u32,
		owner: T::AccountId,
		grantee: T::AccountId,
	) -> DispatchResult {
		let players: Vec<T::AccountId> = vec![owner.clone(), grantee.clone()];
		
		let access_condition = AccessConditionOf::<T> {
			nonce: nonce,
			players: players,
			seq_num: 0,
			status: AppStatus::IDLE,
			outcome: false,
			owner: owner.clone(),
			grantee: grantee.clone(),
		};
		
		<AccessConditionList<T>>::insert(&condition_address, &access_condition);

		Self::deposit_event(
			RawEvent::AccessConditionCreated(
				condition_address,
				owner,
				grantee,
			)
		);
		
		Ok(())
	}

	/// Check if AppStatus is FINALIZED.
	pub fn is_finalized(condition_address: &T::AccountId) -> bool {
		let access_condition = match Self::condition_list(condition_address) {
			Some(_condition) => _condition,
			None => return false
		};

		let status = access_condition.status;

		if status == AppStatus::FINALIZED {
			return true;
		} else {
			return false;
		}
	}

	/// Check if outcome is true.
	pub fn get_outcome(condition_address: &T::AccountId) -> bool {
		let access_condition = match Self::condition_list(condition_address) {
			Some(_condition) => _condition,
			None => return false
		};

		let outcome = access_condition.outcome;

		if outcome == true {
			return true;
		} else {
			return false;
		}
	}

	/// Check if grantee has data access control rights.
	pub fn check_permissions(identity: T::AccountId, grantee: T::AccountId) -> bool {
		if Self::permission(&identity, &grantee) == 1 {
			return true;
		} else {
			return false;
		}
	}

	/// Get nonce of Access Condition.
	pub fn get_nonce(condition_address: T::AccountId) -> Option<u32> {
		let access_condition = match Self::condition_list(&condition_address) {
			Some(_condition) => _condition,
			None => return None
		};

		return Some(access_condition.nonce);
	}

	/// Get sequence number of Access Condition.
	pub fn get_seq_num(condition_address: T::AccountId) -> Option<u32> {
		let access_condition = match Self::condition_list(&condition_address) {
			Some(_condition) => _condition,
			None => return None
		};

		return Some(access_condition.seq_num);
	}

	/// Get AppStatus of Access Condition.
	/// If possible, this function return AppStatus
	pub fn get_status(condition_address: T::AccountId) -> Option<AppStatus> {
		let access_condition = match Self::condition_list(&condition_address) {
			Some(_condition) => _condition,
			None => return None
		};
		
		if access_condition.status == AppStatus::IDLE {
			return Some(AppStatus::IDLE);
		} else {
			return Some(AppStatus::FINALIZED);
		}
	}

	/// Get Owner of Access Condition.
	pub fn get_owner(condition_address: T::AccountId) -> Option<T::AccountId> {
		let access_condition = match Self::condition_list(&condition_address) {
			Some(_condition) => _condition,
			None => return None
		};

		return Some(access_condition.owner);
	}

	/// Get Grantee of Access Condition.
	pub fn get_grantee(condition_address: T::AccountId) -> Option<T::AccountId> {
		let access_condition = match Self::condition_list(&condition_address) {
			Some(_condition) => _condition,
			None => return None
		};

		return Some(access_condition.grantee);
	}

}

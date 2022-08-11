// Copyright (C) 2019-2022 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::ledger::{
    map::{memory_map::MemoryMap, Map, MapRead},
    transition::{Input, Origin},
};
use console::{
    network::prelude::*,
    program::{Ciphertext, Plaintext},
    types::Field,
};

use anyhow::Result;
use std::borrow::Cow;

/// A trait for transition input store.
pub trait InputStorage<N: Network>: Clone + Sync {
    /// The mapping of `transition ID` to `input IDs`.
    type IDMap: for<'a> Map<'a, N::TransitionID, Vec<Field<N>>>;
    /// The mapping of `input ID` to `transition ID`.
    type ReverseIDMap: for<'a> Map<'a, Field<N>, N::TransitionID>;
    /// The mapping of `plaintext hash` to `(optional) plaintext`.
    type ConstantMap: for<'a> Map<'a, Field<N>, Option<Plaintext<N>>>;
    /// The mapping of `plaintext hash` to `(optional) plaintext`.
    type PublicMap: for<'a> Map<'a, Field<N>, Option<Plaintext<N>>>;
    /// The mapping of `ciphertext hash` to `(optional) ciphertext`.
    type PrivateMap: for<'a> Map<'a, Field<N>, Option<Ciphertext<N>>>;
    /// The mapping of `serial number` to `(tag, origin)`.
    type RecordMap: for<'a> Map<'a, Field<N>, (Field<N>, Origin<N>)>;
    /// The mapping of `tag` to `serial number`.
    type RecordTagMap: for<'a> Map<'a, Field<N>, Field<N>>;
    /// The mapping of `external commitment` to `()`. Note: This is **not** the record commitment.
    type ExternalRecordMap: for<'a> Map<'a, Field<N>, ()>;

    /// Initializes the transition input store.
    fn open() -> Self;

    /// Returns the ID map.
    fn id_map(&self) -> &Self::IDMap;
    /// Returns the reverse ID map.
    fn reverse_id_map(&self) -> &Self::ReverseIDMap;
    /// Returns the constant map.
    fn constant_map(&self) -> &Self::ConstantMap;
    /// Returns the public map.
    fn public_map(&self) -> &Self::PublicMap;
    /// Returns the private map.
    fn private_map(&self) -> &Self::PrivateMap;
    /// Returns the record map.
    fn record_map(&self) -> &Self::RecordMap;
    /// Returns the record tag map.
    fn record_tag_map(&self) -> &Self::RecordTagMap;
    /// Returns the external record map.
    fn external_record_map(&self) -> &Self::ExternalRecordMap;

    /* Contains */

    /// Returns `true` if the given input ID exists.
    fn contains_input_id(&self, input_id: &Field<N>) -> Result<bool> {
        self.reverse_id_map().contains_key(input_id)
    }

    /// Returns `true` if the given serial number exists.
    fn contains_serial_number(&self, serial_number: &Field<N>) -> Result<bool> {
        self.record_map().contains_key(serial_number)
    }

    /// Returns `true` if the given tag exists.
    fn contains_tag(&self, tag: &Field<N>) -> Result<bool> {
        self.record_tag_map().contains_key(tag)
    }

    /* Find */

    /// Returns the transition ID that contains the given `input ID`.
    fn find_transition_id(&self, input_id: &Field<N>) -> Result<Option<N::TransitionID>> {
        match self.reverse_id_map().get(input_id)? {
            Some(Cow::Borrowed(transition_id)) => Ok(Some(*transition_id)),
            Some(Cow::Owned(transition_id)) => Ok(Some(transition_id)),
            None => Ok(None),
        }
    }

    /* Get */

    /// Returns the input IDs for the given `transition ID`.
    fn get_input_ids(&self, transition_id: &N::TransitionID) -> Result<Vec<Field<N>>> {
        // Retrieve the input IDs.
        match self.id_map().get(transition_id)? {
            Some(Cow::Borrowed(inputs)) => Ok(inputs.to_vec()),
            Some(Cow::Owned(inputs)) => Ok(inputs),
            None => Ok(vec![]),
        }
    }

    /// Returns the input for the given `transition ID`.
    fn get_inputs(&self, transition_id: &N::TransitionID) -> Result<Vec<Input<N>>> {
        // Constructs the input given the input ID and input value.
        macro_rules! into_input {
            (Input::Record($input_id:ident, $input:expr)) => {
                match $input {
                    Cow::Borrowed((tag, origin)) => Input::Record($input_id, *tag, *origin),
                    Cow::Owned((tag, origin)) => Input::Record($input_id, tag, origin),
                }
            };
            (Input::$Variant:ident($input_id:ident, $input:expr)) => {
                match $input {
                    Cow::Borrowed(input) => Input::$Variant($input_id, input.clone()),
                    Cow::Owned(input) => Input::$Variant($input_id, input),
                }
            };
        }

        // A helper function to construct the input given the input ID.
        let construct_input = |input_id| {
            let constant = self.constant_map().get(&input_id)?;
            let public = self.public_map().get(&input_id)?;
            let private = self.private_map().get(&input_id)?;
            let record = self.record_map().get(&input_id)?;
            let external_record = self.external_record_map().get(&input_id)?;

            // Retrieve the input.
            let input = match (constant, public, private, record, external_record) {
                (Some(constant), None, None, None, None) => into_input!(Input::Constant(input_id, constant)),
                (None, Some(public), None, None, None) => into_input!(Input::Public(input_id, public)),
                (None, None, Some(private), None, None) => into_input!(Input::Private(input_id, private)),
                (None, None, None, Some(record), None) => into_input!(Input::Record(input_id, record)),
                (None, None, None, None, Some(_)) => Input::ExternalRecord(input_id),
                (None, None, None, None, None) => bail!("Missing input '{input_id}' in transition '{transition_id}'"),
                _ => bail!("Found multiple inputs for the input ID '{input_id}' in transition '{transition_id}'"),
            };

            Ok(input)
        };

        // Retrieve the input IDs.
        match self.id_map().get(transition_id)? {
            Some(Cow::Borrowed(ids)) => ids.iter().map(|input_id| construct_input(*input_id)).collect(),
            Some(Cow::Owned(ids)) => ids.iter().map(|input_id| construct_input(*input_id)).collect(),
            None => Ok(vec![]),
        }
    }

    /* Iterators */

    /// Returns an iterator over the input IDs, for all transition inputs.
    fn input_ids(&self) -> <Self::ReverseIDMap as MapRead<Field<N>, N::TransitionID>>::Keys {
        self.reverse_id_map().keys()
    }

    /// Returns an iterator over the constant input IDs, for all transition inputs that are constant.
    fn constant_input_ids(&self) -> <Self::ConstantMap as MapRead<Field<N>, Option<Plaintext<N>>>>::Keys {
        self.constant_map().keys()
    }

    /// Returns an iterator over the public input IDs, for all transition inputs that are public.
    fn public_input_ids(&self) -> <Self::PublicMap as MapRead<Field<N>, Option<Plaintext<N>>>>::Keys {
        self.public_map().keys()
    }

    /// Returns an iterator over the private input IDs, for all transition inputs that are private.
    fn private_input_ids(&self) -> <Self::PrivateMap as MapRead<Field<N>, Option<Ciphertext<N>>>>::Keys {
        self.private_map().keys()
    }

    /// Returns an iterator over the serial numbers, for all transition inputs that are records.
    fn serial_numbers(&self) -> <Self::RecordMap as MapRead<Field<N>, (Field<N>, Origin<N>)>>::Keys {
        self.record_map().keys()
    }

    /// Returns an iterator over the external record input IDs, for all transition inputs that are external records.
    fn external_input_ids(&self) -> <Self::ExternalRecordMap as MapRead<Field<N>, ()>>::Keys {
        self.external_record_map().keys()
    }

    /// Returns an iterator over the constant inputs, for all transitions.
    fn constant_inputs(&self) -> <Self::ConstantMap as MapRead<Field<N>, Option<Plaintext<N>>>>::Values {
        self.constant_map().values().flat_map(|input| match input {
            Cow::Borrowed(Some(input)) => Some(Cow::Borrowed(input)),
            Cow::Owned(Some(input)) => Some(Cow::Owned(input)),
            _ => None,
        })
    }

    /// Returns an iterator over the constant inputs, for all transitions.
    fn public_inputs(&self) -> <Self::PublicMap as MapRead<Field<N>, Option<Plaintext<N>>>>::Values {
        self.public_map().values().flat_map(|input| match input {
            Cow::Borrowed(Some(input)) => Some(Cow::Borrowed(input)),
            Cow::Owned(Some(input)) => Some(Cow::Owned(input)),
            _ => None,
        })
    }

    /// Returns an iterator over the private inputs, for all transitions.
    fn private_inputs(&self) -> <Self::PrivateMap as MapRead<Field<N>, Option<Ciphertext<N>>>>::Values {
        self.private_map().values().flat_map(|input| match input {
            Cow::Borrowed(Some(input)) => Some(Cow::Borrowed(input)),
            Cow::Owned(Some(input)) => Some(Cow::Owned(input)),
            _ => None,
        })
    }

    /// Returns an iterator over the tags, for all transition inputs that are records.
    fn tags(&self) -> <Self::RecordTagMap as MapRead<Field<N>, Field<N>>>::Keys {
        self.record_tag_map().keys()
    }

    /// Returns an iterator over the origins, for all transition inputs that are records.
    fn origins(&self) -> <Self::RecordMap as MapRead<Field<N>, (Field<N>, Origin<N>)>>::Values {
        self.record_map().values().map(|input| match input {
            Cow::Borrowed((_, origin)) => Cow::Borrowed(origin),
            Cow::Owned((_, origin)) => Cow::Owned(origin),
        })
    }

    /* Write */

    /// Stores the given `(transition ID, input)` pair into storage.
    fn insert(&self, transition_id: N::TransitionID, inputs: &[Input<N>]) -> Result<()> {
        // Store the input IDs.
        self.id_map().insert(transition_id, inputs.iter().map(Input::id).copied().collect())?;

        // Store the inputs.
        for input in inputs {
            // Store the reverse input ID.
            self.reverse_id_map().insert(*input.id(), transition_id)?;
            // Store the input.
            match input.clone() {
                Input::Constant(input_id, constant) => self.constant_map().insert(input_id, constant)?,
                Input::Public(input_id, public) => self.public_map().insert(input_id, public)?,
                Input::Private(input_id, private) => self.private_map().insert(input_id, private)?,
                Input::Record(serial_number, tag, origin) => {
                    // Store the record tag.
                    self.record_tag_map().insert(tag, serial_number)?;
                    // Store the record.
                    self.record_map().insert(serial_number, (tag, origin))?
                }
                Input::ExternalRecord(input_id) => self.external_record_map().insert(input_id, ())?,
            }
        }
        Ok(())
    }

    /// Removes the input for the given `transition ID`.
    fn remove(&self, transition_id: &N::TransitionID) -> Result<()> {
        // Retrieve the input IDs.
        let input_ids: Vec<_> = match self.id_map().get(transition_id)? {
            Some(Cow::Borrowed(ids)) => ids.to_vec(),
            Some(Cow::Owned(ids)) => ids.into_iter().collect(),
            None => return Ok(()),
        };

        // Remove the input IDs.
        self.id_map().remove(transition_id)?;

        // Remove the inputs.
        for input_id in input_ids {
            // Remove the reverse input ID.
            self.reverse_id_map().remove(&input_id)?;

            // If the input is a record, remove the record tag.
            if let Some(record) = self.record_map().get(&input_id)? {
                self.record_tag_map().remove(&record.0)?;
            }

            // Remove the input.
            self.constant_map().remove(&input_id)?;
            self.public_map().remove(&input_id)?;
            self.private_map().remove(&input_id)?;
            self.record_map().remove(&input_id)?;
            self.external_record_map().remove(&input_id)?;
        }

        Ok(())
    }
}

/// An in-memory transition input store.
#[derive(Clone)]
pub struct InputMemory<N: Network> {
    /// The mapping of `transition ID` to `input IDs`.
    id_map: MemoryMap<N::TransitionID, Vec<Field<N>>>,
    /// The mapping of `input ID` to `transition ID`.
    reverse_id_map: MemoryMap<Field<N>, N::TransitionID>,
    /// The mapping of `plaintext hash` to `(optional) plaintext`.
    constant: MemoryMap<Field<N>, Option<Plaintext<N>>>,
    /// The mapping of `plaintext hash` to `(optional) plaintext`.
    public: MemoryMap<Field<N>, Option<Plaintext<N>>>,
    /// The mapping of `ciphertext hash` to `(optional) ciphertext`.
    private: MemoryMap<Field<N>, Option<Ciphertext<N>>>,
    /// The mapping of `serial number` to `(tag, origin)`.
    record: MemoryMap<Field<N>, (Field<N>, Origin<N>)>,
    /// The mapping of `record tag` to `serial number`.
    record_tag: MemoryMap<Field<N>, Field<N>>,
    /// The mapping of `external commitment` to `()`. Note: This is **not** the record commitment.
    external_record: MemoryMap<Field<N>, ()>,
}

#[rustfmt::skip]
impl<N: Network> InputStorage<N> for InputMemory<N> {
    type IDMap = MemoryMap<N::TransitionID, Vec<Field<N>>>;
    type ReverseIDMap = MemoryMap<Field<N>, N::TransitionID>;
    type ConstantMap = MemoryMap<Field<N>, Option<Plaintext<N>>>;
    type PublicMap = MemoryMap<Field<N>, Option<Plaintext<N>>>;
    type PrivateMap = MemoryMap<Field<N>, Option<Ciphertext<N>>>;
    type RecordMap = MemoryMap<Field<N>, (Field<N>, Origin<N>)>;
    type RecordTagMap = MemoryMap<Field<N>, Field<N>>;
    type ExternalRecordMap = MemoryMap<Field<N>, ()>;

    /// Initializes the transition input store.
    fn open() -> Self {
        Self {
            id_map: MemoryMap::default(),
            reverse_id_map: MemoryMap::default(),
            constant: MemoryMap::default(),
            public: MemoryMap::default(),
            private: MemoryMap::default(),
            record: MemoryMap::default(),
            record_tag: MemoryMap::default(),
            external_record: MemoryMap::default(),
        }
    }

    /// Returns the ID map.
    fn id_map(&self) -> &Self::IDMap {
        &self.id_map
    }

    /// Returns the reverse ID map.
    fn reverse_id_map(&self) -> &Self::ReverseIDMap {
        &self.reverse_id_map
    }

    /// Returns the constant map.
    fn constant_map(&self) -> &Self::ConstantMap {
        &self.constant
    }

    /// Returns the public map.
    fn public_map(&self) -> &Self::PublicMap {
        &self.public
    }

    /// Returns the private map.
    fn private_map(&self) -> &Self::PrivateMap {
        &self.private
    }

    /// Returns the record map.
    fn record_map(&self) -> &Self::RecordMap {
        &self.record
    }

    /// Returns the record tag map.
    fn record_tag_map(&self) -> &Self::RecordTagMap {
        &self.record_tag
    }

    /// Returns the external record map.
    fn external_record_map(&self) -> &Self::ExternalRecordMap {
        &self.external_record
    }
}

/// The transition input store.
#[derive(Clone)]
pub struct InputStore<N: Network, I: InputStorage<N>> {
    /// The map of constant inputs.
    constant: I::ConstantMap,
    /// The map of public inputs.
    public: I::PublicMap,
    /// The map of private inputs.
    private: I::PrivateMap,
    /// The map of record inputs.
    record: I::RecordMap,
    /// The map of record tags.
    record_tag: I::RecordTagMap,
    /// The map of external record inputs.
    external_record: I::ExternalRecordMap,
    /// The input storage.
    storage: I,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_get_remove() {
        // Sample the transition inputs.
        for (transition_id, input) in crate::ledger::transition::input::test_helpers::sample_inputs() {
            // Initialize a new input store.
            let input_store = InputMemory::open();

            // Ensure the transition input does not exist.
            let candidate = input_store.get_inputs(&transition_id).unwrap();
            assert!(candidate.is_empty());

            // Insert the transition input.
            input_store.insert(transition_id, &[input.clone()]).unwrap();

            // Retrieve the transition input.
            let candidate = input_store.get_inputs(&transition_id).unwrap();
            assert_eq!(vec![input.clone()], candidate);

            // Remove the transition input.
            input_store.remove(&transition_id).unwrap();

            // Retrieve the transition input.
            let candidate = input_store.get_inputs(&transition_id).unwrap();
            assert!(candidate.is_empty());
        }
    }

    #[test]
    fn test_find_transition_id() {
        // Sample the transition inputs.
        for (transition_id, input) in crate::ledger::transition::input::test_helpers::sample_inputs() {
            // Initialize a new input store.
            let input_store = InputMemory::open();

            // Ensure the transition input does not exist.
            let candidate = input_store.get_inputs(&transition_id).unwrap();
            assert!(candidate.is_empty());

            // Ensure the transition ID is not found.
            let candidate = input_store.find_transition_id(input.id()).unwrap();
            assert!(candidate.is_none());

            // Insert the transition input.
            input_store.insert(transition_id, &[input.clone()]).unwrap();

            // Find the transition ID.
            let candidate = input_store.find_transition_id(input.id()).unwrap();
            assert_eq!(Some(transition_id), candidate);

            // Remove the transition input.
            input_store.remove(&transition_id).unwrap();

            // Ensure the transition ID is not found.
            let candidate = input_store.find_transition_id(input.id()).unwrap();
            assert!(candidate.is_none());
        }
    }
}

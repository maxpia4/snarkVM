// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use console::network::prelude::{Deserialize, Result, Serialize};

use core::{borrow::Borrow, hash::Hash};
use std::borrow::Cow;

/// A trait representing map-like storage operations with read-write capabilities.
pub trait Map<
    'a,
    K: 'a + Copy + Clone + PartialEq + Eq + Hash + Serialize + Deserialize<'a> + Send + Sync,
    V: 'a + Clone + PartialEq + Eq + Serialize + Deserialize<'a> + Send + Sync,
>: Clone + MapRead<'a, K, V> + Send + Sync
{
    ///
    /// Inserts the given key-value pair into the map.
    ///
    fn insert(&self, key: K, value: V) -> Result<()>;

    ///
    /// Removes the key-value pair for the given key from the map.
    ///
    fn remove(&self, key: &K) -> Result<()>;

    ///
    /// Begins an atomic operation. Any further calls to `insert` and `remove` will be queued
    /// without an actual write taking place until `finish_atomic` is called.
    ///
    fn start_atomic(&self);

    ///
    /// Checks whether an atomic operation is currently in progress. This can be done to ensure
    /// that lower-level operations don't start or finish their individual atomic write batch
    /// if they are already part of a larger one.
    ///
    fn is_atomic_in_progress(&self) -> bool;

    ///
    /// Saves the current list of pending operations, so that if `atomic_rewind` is called,
    /// we roll back all future operations, and return to the start of this checkpoint.
    ///
    fn atomic_checkpoint(&self);

    ///
    /// Removes the latest atomic checkpoint.
    ///
    fn clear_latest_checkpoint(&self);

    ///
    /// Removes all pending operations to the last `atomic_checkpoint`
    /// (or to `start_atomic` if no checkpoints have been created).
    ///
    fn atomic_rewind(&self);

    ///
    /// Aborts the current atomic operation.
    ///
    fn abort_atomic(&self);

    ///
    /// Finishes an atomic operation, performing all the queued writes.
    ///
    fn finish_atomic(&self) -> Result<()>;
}

/// A trait representing map-like storage operations with read-only capabilities.
pub trait MapRead<
    'a,
    K: 'a + Copy + Clone + PartialEq + Eq + Hash + Serialize + Deserialize<'a> + Sync,
    V: 'a + Clone + PartialEq + Eq + Serialize + Deserialize<'a> + Sync,
>
{
    type PendingIterator: Iterator<Item = (Cow<'a, K>, Option<Cow<'a, V>>)>;
    type Iterator: Iterator<Item = (Cow<'a, K>, Cow<'a, V>)>;
    type Keys: Iterator<Item = Cow<'a, K>>;
    type Values: Iterator<Item = Cow<'a, V>>;

    ///
    /// Returns `true` if the given key exists in the map.
    ///
    fn contains_key_confirmed<Q>(&self, key: &Q) -> Result<bool>
    where
        K: Borrow<Q>,
        Q: PartialEq + Eq + Hash + Serialize + ?Sized;

    ///
    /// Returns `true` if the given key exists in the map.
    /// This method first checks the atomic batch, and if it does not exist, then checks the map.
    ///
    fn contains_key_speculative<Q>(&self, key: &Q) -> Result<bool>
    where
        K: Borrow<Q>,
        Q: PartialEq + Eq + Hash + Serialize + ?Sized;

    ///
    /// Returns the value for the given key from the map, if it exists.
    ///
    fn get_confirmed<Q>(&'a self, key: &Q) -> Result<Option<Cow<'a, V>>>
    where
        K: Borrow<Q>,
        Q: PartialEq + Eq + Hash + Serialize + ?Sized;

    ///
    /// Returns the current value for the given key if it is scheduled
    /// to be inserted as part of an atomic batch.
    ///
    /// If the key does not exist, returns `None`.
    /// If the key is removed in the batch, returns `Some(None)`.
    /// If the key is inserted in the batch, returns `Some(Some(value))`.
    ///
    fn get_pending<Q>(&self, key: &Q) -> Option<Option<V>>
    where
        K: Borrow<Q>,
        Q: PartialEq + Eq + Hash + Serialize + ?Sized;

    ///
    /// Returns the value for the given key from the atomic batch first, if it exists,
    /// or return from the map, otherwise.
    ///
    fn get_speculative<Q>(&'a self, key: &Q) -> Result<Option<Cow<'a, V>>>
    where
        K: Borrow<Q>,
        Q: PartialEq + Eq + Hash + Serialize + ?Sized,
    {
        // Return early in case of errors in order to not conceal them.
        let map_value = self.get_confirmed(key)?;

        // Retrieve the atomic batch value, if it exists.
        let atomic_batch_value = self.get_pending(key);

        // Return the atomic batch value, if it exists, or the map value, otherwise.
        match atomic_batch_value {
            Some(Some(value)) => Ok(Some(Cow::Owned(value))),
            Some(None) => Ok(None),
            None => Ok(map_value),
        }
    }

    ///
    /// Returns an iterator visiting each key-value pair in the atomic batch.
    ///
    fn iter_pending(&'a self) -> Self::PendingIterator;

    ///
    /// Returns an iterator visiting each key-value pair in the map.
    ///
    fn iter_confirmed(&'a self) -> Self::Iterator;

    ///
    /// Returns an iterator over each key in the map.
    ///
    fn keys_confirmed(&'a self) -> Self::Keys;

    ///
    /// Returns an iterator over each value in the map.
    ///
    fn values_confirmed(&'a self) -> Self::Values;
}

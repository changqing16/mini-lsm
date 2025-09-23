// Copyright (c) 2022-2025 Alex Chi Z
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use std::{sync::Arc, thread::current};

use anyhow::Result;

use super::StorageIterator;
use crate::{
    key::KeySlice,
    table::{SsTable, SsTableIterator},
};

/// Concat multiple iterators ordered in key order and their key ranges do not overlap. We do not want to create the
/// iterators when initializing this iterator to reduce the overhead of seeking.
pub struct SstConcatIterator {
    current: Option<SsTableIterator>,
    next_sst_idx: usize,
    sstables: Vec<Arc<SsTable>>,
}

impl SstConcatIterator {
    fn check_sst_valid(sstables: &[Arc<SsTable>]) {
        for sst in sstables {
            assert!(sst.first_key() <= sst.last_key());
        }
        if !sstables.is_empty() {
            for i in 0..(sstables.len() - 1) {
                assert!(sstables[i].last_key() < sstables[i + 1].first_key());
            }
        }
    }

    pub fn create_and_seek_to_first(sstables: Vec<Arc<SsTable>>) -> Result<Self> {
        Self::check_sst_valid(&sstables);

        if sstables.is_empty() {
            return Ok(Self {
                current: None,
                next_sst_idx: 0,
                sstables,
            });
        }

        let iter = SsTableIterator::create_and_seek_to_first(sstables[0].clone())?;
        Ok(Self {
            current: Some(iter),
            next_sst_idx: 1,
            sstables,
        })
    }

    pub fn create_and_seek_to_key(sstables: Vec<Arc<SsTable>>, key: KeySlice) -> Result<Self> {
        Self::check_sst_valid(&sstables);

        let idx = sstables
            .partition_point(|x| x.first_key().as_key_slice() <= key)
            .saturating_sub(1);

        if idx >= sstables.len() {
            return Ok(Self {
                current: None,
                next_sst_idx: 0,
                sstables,
            });
        }

        let iter = SsTableIterator::create_and_seek_to_key(sstables[idx].clone(), key)?;
        Ok(Self {
            current: Some(iter),
            next_sst_idx: idx + 1,
            sstables,
        })
    }
}

impl StorageIterator for SstConcatIterator {
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice {
        self.current.as_ref().unwrap().key()
    }

    fn value(&self) -> &[u8] {
        self.current.as_ref().unwrap().value()
    }

    fn is_valid(&self) -> bool {
        self.current.as_ref().map_or(false, |x| x.is_valid())
    }

    fn next(&mut self) -> Result<()> {
        let current = self.current.as_mut().unwrap();
        current.next()?;
        if !current.is_valid() && self.next_sst_idx < self.sstables.len() {
            let next = SsTableIterator::create_and_seek_to_first(
                self.sstables[self.next_sst_idx].clone(),
            )?;
            self.next_sst_idx += 1;
            _ = std::mem::replace(current, next);
        }

        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        1
    }
}

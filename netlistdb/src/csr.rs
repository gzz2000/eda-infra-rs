//! Compressed sparse row (CSR) implementation.

use ulib::UVec;

/// A helper type for simple 1-layer CSR.
#[derive(Debug, Default, Clone)]
pub struct VecCSR {
    /// flattened list start, analogous to `flat_net2pin_start`
    pub start: UVec<usize>,
    /// flattened list, analogous to `netpin`, indexed by [VecCSR::start]
    pub items: UVec<usize>,
}

impl VecCSR {
    /// build CSR from the mapping between items to set indices.
    pub fn from(num_sets: usize, num_items: usize, inset: &[usize]) -> VecCSR {
        assert_eq!(inset.len(), num_items);
        let mut start: Vec<usize> = vec![0; num_sets + 1];
        let mut items: Vec<usize> = vec![0; num_items];
        for s in inset {
            start[*s] += 1;
        }
        // todo: parallelizable
        for i in 1..num_sets + 1 {
            start[i] += start[i - 1];
        }
        assert_eq!(start[num_sets], num_items);
        // todo: parallelizable
        for i in (0..num_items).rev() {
            let s = inset[i];
            let pos = start[s] - 1;
            start[s] -= 1;
            items[pos] = i;
        }
        VecCSR {
            start: start.into(),
            items: items.into()
        }
    }

    /// convenient method to get an iterator of set items.
    #[inline]
    pub fn iter_set(&self, set_id: usize)
                    -> impl Iterator<Item = usize> + '_
    {
        let l = self.start[set_id];
        let r = self.start[set_id + 1];
        self.items[l..r].iter().copied()
    }
    
    /// get size of a set
    #[inline]
    pub fn len(&self, set_id: usize) -> usize {
        let l = self.start[set_id];
        let r = self.start[set_id + 1];
        r - l
    }
}

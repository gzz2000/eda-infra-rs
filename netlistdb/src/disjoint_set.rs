//! Union-find set implementation for net discovery.

/// A simple implementation of a union-find set (disjoint set).
/// We extend it with the ability to track two special sets:
/// Net-0 and Net-1.
/// 
/// It is expected to run in `O(log n)` time, because we do not
/// use *union by size/rank* trick to optimize it -- this is
/// hopefully because nets are small.
///
/// One do not need to manually add nodes. New nodes with new
/// indices are automatically added.
pub struct DisjointSet {
    fa: Vec<usize>,
    value_zero: Option<usize>,
    value_one: Option<usize>
}

impl DisjointSet {
    /// Create a disjoint set with certain pre-allocated
    /// memory space.
    pub fn with_capacity(c: usize) -> DisjointSet {
        DisjointSet{
            fa: Vec::with_capacity(c),
            value_zero: None,
            value_one: None
        }
    }

    /// Find the current group leader of a node.
    fn find(&mut self, mut u: usize) -> usize {
        if self.fa.len() <= u {
            // insert necessary elems with group leader
            // pointing to themselves. easy to do using iters.
            self.fa.extend(self.fa.len()..=u);
        }
        let mut s = u;
        while self.fa[s] != s {
            s = self.fa[s];
        }
        while self.fa[u] != s {
            let t = self.fa[u];
            self.fa[u] = s;
            u = t;
        }
        s
    }

    /// Join (merge) two sets.
    pub fn merge(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.fa[a] = b;
    }

    /// Set value to be zero or one
    pub fn set_value(&mut self, a: usize, v: bool) {
        macro_rules! cas {
            ($($v:expr => $field:ident),+) => {
                match v {
                    $($v => match self.$field {
                        None => self.$field = Some(a),
                        Some(b) => self.merge(a, b)
                    }),+
                }
            }
        }
        cas! {
            false => value_zero,
            true => value_one
        }
    }

    /// Finalize and output the number of sets, the set indices
    /// of all nodes, and the zero/one set indices.
    ///
    /// Currently, this consumes the whole disjoint set object to
    /// warn user that it is an expensive operation.
    pub fn finalize(mut self, num_nodes: usize) -> Option<(
        usize, Vec<usize>,
        Option<usize>, Option<usize>
    )> {
        self.fa.truncate(num_nodes);
        self.fa.extend(self.fa.len()..num_nodes);

        let mut set_indices = vec![0; num_nodes];
        let mut num_sets = 0;
        for (i, f) in self.fa.iter().enumerate() {
            if i == *f {
                set_indices[i] = num_sets;
                num_sets += 1;
            }
        }
        for i in 0..self.fa.len() {
            if i != self.fa[i] {
                set_indices[i] = set_indices[self.find(i)];
            }
        }

        let id_zero = self.value_zero.map(|i| set_indices[i]);
        let id_one = self.value_one.map(|i| set_indices[i]);
        if matches!((id_zero, id_one), (Some(a), Some(b)) if a == b) {
            clilog::error!(NL_SV_LIT, "Constant zero and one connected");
            return None
        }
        
        Some((num_sets, set_indices, id_zero, id_one))
    }
}


//! An inclusive range implementation for verilog.

/// An inclusive range in verilog.
/// 
/// The direction is inferred from the relation between two ends.
/// As the pair of sizes cannot represent *empty*, we use
/// [`isize::MAX`]:[`isize::MAX`] to represent an empty range.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SVerilogRange(pub isize, pub isize);

impl SVerilogRange {
    #[inline]
    pub fn empty() -> SVerilogRange {
        SVerilogRange(isize::MAX, isize::MAX)
    }

    #[inline]
    pub fn get_len(&self) -> usize {
        if *self == SVerilogRange::empty() {
            return 0;
        }
        let (l, r) = {
            if self.0 < self.1 { (self.0, self.1) }
            else { (self.1, self.0) }
        };
        (r + 1 - l).try_into().unwrap()
    }
}

impl Iterator for SVerilogRange {
    type Item = isize;

    #[inline]
    fn next(&mut self) -> Option<isize> {
        if *self == SVerilogRange::empty() {
            return None
        }
        let ret = self.0;
        if self.0 < self.1 {
            self.0 += 1;
        }
        else if self.0 > self.1 {
            self.0 -= 1;
        }
        else {
            *self = SVerilogRange::empty();
        }
        Some(ret)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.get_len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for SVerilogRange {
    #[inline]
    fn len(&self) -> usize {
        self.get_len()
    }
}

#[test]
fn test_range() {
    assert_eq!(SVerilogRange(-2, 99).len(), 102);
    assert_eq!(SVerilogRange(99, -2).len(), 102);
    assert_eq!(SVerilogRange(0, 0).len(), 1);
    assert_eq!(SVerilogRange(1, 6).collect::<Vec<_>>(),
               vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(SVerilogRange(4, -3).collect::<Vec<_>>(),
               vec![4, 3, 2, 1, 0, -1, -2, -3]);
}

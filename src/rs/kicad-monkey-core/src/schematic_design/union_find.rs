pub(super) struct UnionFind {
    parents: Vec<usize>,
    ranks: Vec<u8>,
}

impl UnionFind {
    pub(super) fn new(count: usize) -> Self {
        Self {
            parents: (0..count).collect(),
            ranks: vec![0; count],
        }
    }

    pub(super) fn find(&mut self, mut value: usize) -> usize {
        while self.parents[value] != value {
            self.parents[value] = self.parents[self.parents[value]];
            value = self.parents[value];
        }
        value
    }

    pub(super) fn union(&mut self, left: usize, right: usize) {
        let mut left = self.find(left);
        let mut right = self.find(right);
        if left == right {
            return;
        }
        if self.ranks[left] < self.ranks[right] {
            std::mem::swap(&mut left, &mut right);
        }
        self.parents[right] = left;
        if self.ranks[left] == self.ranks[right] {
            self.ranks[left] = self.ranks[left].saturating_add(1);
        }
    }
}

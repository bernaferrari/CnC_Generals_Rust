#[derive(Clone)]
struct NodeElement<T> {
    id: i32,
    data: T,
}

pub struct IndexClass<T>
where
    T: Clone + Default,
{
    table: Vec<NodeElement<T>>,
    index_count: usize,
    index_size: usize,
    is_sorted: bool,
    archive_id: Option<i32>,
    archive_index: Option<usize>,
}

impl<T> IndexClass<T>
where
    T: Clone + Default,
{
    pub fn new() -> Self {
        Self {
            table: Vec::new(),
            index_count: 0,
            index_size: 0,
            is_sorted: false,
            archive_id: None,
            archive_index: None,
        }
    }

    pub fn clear(&mut self) {
        self.table.clear();
        self.index_count = 0;
        self.index_size = 0;
        self.is_sorted = false;
        self.invalidate_archive();
    }

    pub fn add_index(&mut self, id: i32, data: T) -> bool {
        if self.index_count + 1 > self.index_size {
            let grow = if self.index_size == 0 {
                10
            } else {
                self.index_size
            };
            if !self.increase_table_size(grow) {
                return false;
            }
        }

        self.table.push(NodeElement { id, data });
        self.index_count += 1;
        self.is_sorted = false;
        true
    }

    pub fn remove_index(&mut self, id: i32) -> bool {
        let mut found = None;
        for (index, node) in self.table.iter().enumerate().take(self.index_count) {
            if node.id == id {
                found = Some(index);
                break;
            }
        }

        if let Some(found_index) = found {
            self.table.remove(found_index);
            self.index_count -= 1;
            self.invalidate_archive();
            return true;
        }

        false
    }

    pub fn is_present(&mut self, id: i32) -> bool {
        if self.index_count == 0 {
            return false;
        }
        if self.is_archive_same(id) {
            return true;
        }
        if let Some(index) = self.search_for_node(id) {
            self.set_archive(id, index);
            return true;
        }
        false
    }

    pub fn fetch_index(&mut self, id: i32) -> T {
        if self.is_present(id) {
            if let Some(index) = self.archive_index {
                return self.table[index].data.clone();
            }
        }
        T::default()
    }

    pub fn count(&self) -> usize {
        self.index_count
    }

    fn increase_table_size(&mut self, amount: usize) -> bool {
        self.index_size += amount;
        self.table.reserve(amount);
        self.invalidate_archive();
        true
    }

    fn is_archive_same(&self, id: i32) -> bool {
        match (self.archive_id, self.archive_index) {
            (Some(archive_id), Some(index)) => archive_id == id && index < self.index_count,
            _ => false,
        }
    }

    fn invalidate_archive(&mut self) {
        self.archive_id = None;
        self.archive_index = None;
    }

    fn set_archive(&mut self, id: i32, index: usize) {
        self.archive_id = Some(id);
        self.archive_index = Some(index);
    }

    fn search_for_node(&mut self, id: i32) -> Option<usize> {
        if self.index_count == 0 {
            return None;
        }
        if !self.is_sorted {
            self.table[..self.index_count].sort_by_key(|node| node.id);
            self.invalidate_archive();
            self.is_sorted = true;
        }
        let slice = &self.table[..self.index_count];
        slice.binary_search_by(|node| node.id.cmp(&id)).ok()
    }
}

impl<T> Default for IndexClass<T>
where
    T: Clone + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

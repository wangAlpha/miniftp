use std::borrow::Borrow;
use std::boxed::Box;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem::{self, MaybeUninit};
use std::ptr;
use std::time::Instant;

pub struct TimerList<K: Hash + Eq, V> {
    list: SortedList<K, (Instant, V)>,
    timeout: u64,
}

impl<K: Hash + Eq, V> TimerList<K, V> {
    pub fn new(timeout: u64) -> Self {
        TimerList {
            list: SortedList::new(),
            timeout,
        }
    }
    pub fn len(&self) -> usize {
        self.list.len()
    }
    pub fn contains(&mut self, k: &K) -> bool {
        self.list.contains(&k)
    }
    pub fn insert(&mut self, k: K, v: V) {
        self.list.put(k, (Instant::now(), v));
    }
    pub fn get<'a>(&'a mut self, k: &K) -> Option<&'a V> {
        match self.list.get(k) {
            Some((_, v)) => Some(v),
            None => None,
        }
    }
    pub fn get_mut<'a>(&'a mut self, k: &K) -> Option<&'a V> {
        match self.list.get_mut(k) {
            Some((_, v)) => Some(v),
            None => None,
        }
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        match self.list.remove(k) {
            Some((_, v)) => Some(v),
            None => None,
        }
    }
    pub fn remove_idle(&mut self) {
        while !self.list.is_empty() {
            match self.list.last() {
                Some((instant, _)) => {
                    if self.timeout > instant.elapsed().as_secs() {
                        // debug!("Idle node: {}, time: {}", node, instant.elapsed().as_secs());
                        self.list.remove_last();
                    }
                }
                None => break,
            }
        }
    }
}

// Borrowing rule restrictions, use pointers to store keys
pub struct KeyRef<K> {
    k: *const K,
}

impl<K: Hash> Hash for KeyRef<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe { (*self.k).hash(state) }
    }
}

impl<K: PartialEq> PartialEq for KeyRef<K> {
    fn eq(&self, other: &KeyRef<K>) -> bool {
        unsafe { (*self.k).eq(&*other.k) }
    }
}
impl<K: Eq> Eq for KeyRef<K> {}

impl<K> Borrow<K> for KeyRef<K> {
    fn borrow(&self) -> &K {
        unsafe { &*self.k }
    }
}

// K/V is stored on the node of the bidirectional list
struct Entry<K, V> {
    key: K,
    val: V,
    prev: *mut Entry<K, V>,
    next: *mut Entry<K, V>,
}

impl<K, V> Entry<K, V> {
    fn new(key: K, val: V) -> Self {
        Entry {
            key,
            val,
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
        }
    }
}
pub struct SortedList<K, V> {
    map: HashMap<KeyRef<K>, Box<Entry<K, V>>>,
    head: *mut Entry<K, V>,
    tail: *mut Entry<K, V>,
}

impl<K: Hash + Eq, V> SortedList<K, V> {
    // Box::into_raw converts the smart pointer to a raw pointer,
    // and SortedList is responsible for freeing the corresponding memory.
    // The way to free memory is to use from_raw, which converts the raw pointer into a Box.
    pub fn new() -> Self {
        let list = SortedList {
            map: HashMap::new(),
            head: unsafe {
                Box::into_raw(Box::new(MaybeUninit::<Entry<K, V>>::uninit().assume_init()))
            },
            tail: unsafe {
                Box::into_raw(Box::new(MaybeUninit::<Entry<K, V>>::uninit().assume_init()))
            },
        };
        unsafe {
            (*list.head).next = list.tail;
            (*list.head).prev = list.head;

            (*list.tail).prev = list.head;
            (*list.tail).next = list.tail;
        }
        list
    }

    // update Value, else return None
    pub fn put(&mut self, k: K, mut v: V) -> Option<V> {
        // Find node from map
        let node_ptr = self.map.get_mut(&KeyRef { k: &k }).map(|node| {
            let node_ptr: *mut Entry<K, V> = &mut **node;
            node_ptr
        });
        match node_ptr {
            Some(node_ptr) => {
                // If the node exists, update and return it
                unsafe { mem::swap(&mut v, &mut (*node_ptr).val) }
                self.detach(node_ptr);
                self.attach(node_ptr);
                Some(v)
            }
            None => {
                let mut node = Box::new(Entry::new(k, v));
                // From smart point to raw pointer
                let node_ptr: *mut Entry<K, V> = &mut *node;
                self.attach(node_ptr);
                // Insert new Key/Value to map
                let keyref = unsafe { &(*node_ptr).key };
                self.map.insert(KeyRef { k: keyref }, node);
                None
            }
        }
    }
    // Get Value of Key, and Update list
    pub fn get<'a, Q>(&'a mut self, k: &Q) -> Option<&'a V>
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let (node_ptr, value) = match self.map.get_mut(k) {
            None => (None, None),
            Some(node) => {
                // Dereference smart pointer to raw pointer
                let node_ptr: *mut Entry<K, V> = &mut **node;
                (Some(node_ptr), Some(unsafe { &(*node_ptr).val }))
            }
        };
        match node_ptr {
            None => (),
            Some(node_ptr) => {
                // Attach to head
                self.detach(node_ptr);
                self.attach(node_ptr);
            }
        }
        value
    }

    pub fn get_mut<'a>(&'a mut self, k: &K) -> Option<&'a mut V> {
        let key = KeyRef { k };
        let (node_ptr, value) = match self.map.get_mut(&key) {
            None => (None, None),
            Some(node) => {
                let node_ptr: *mut Entry<K, V> = &mut **node;
                (Some(node_ptr), Some(unsafe { &mut (*node_ptr).val }))
            }
        };
        match node_ptr {
            None => (),
            Some(node_ptr) => {
                self.detach(node_ptr);
                self.attach(node_ptr);
            }
        }
        value
    }

    pub fn last<'a>(&'a mut self) -> Option<&'a V> {
        let node = unsafe { (*self.tail).prev };
        unsafe { Some(&(*node).val) }
    }

    pub fn front<'a>(&'a mut self) -> Option<&'a V> {
        let node = unsafe { (*self.head).next };
        unsafe { Some(&(*node).val) }
    }

    pub fn contains(&self, k: &K) -> bool {
        let key = KeyRef { k };
        self.map.contains_key(&key)
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let key = KeyRef { k };
        match self.map.remove(&key) {
            None => None,
            Some(mut old_node) => {
                let node_ptr: *mut Entry<K, V> = &mut *old_node;
                self.detach(node_ptr);
                Some(old_node.val)
            }
        }
    }

    // Remove idle nodes at the tail
    pub fn pop(&mut self) -> Option<(K, V)> {
        let node = self.remove_last()?;
        let node = *node;

        let Entry { key, val, .. } = node;
        Some((key, val))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        loop {
            match self.remove_last() {
                Some(_) => (),
                None => break,
            }
        }
    }

    fn remove_last(&mut self) -> Option<Box<Entry<K, V>>> {
        let prev;
        unsafe { prev = (*self.tail).prev }
        // Exist vaild node
        if prev != self.head {
            let old_key = KeyRef {
                k: unsafe { &(*(*self.tail).prev).key },
            };
            let mut old_node = self.map.remove(&old_key).unwrap();
            let node_ptr: *mut Entry<K, V> = &mut *old_node;
            self.detach(node_ptr);
            // Reture smart pointer
            Some(old_node)
        } else {
            None
        }
    }

    fn detach(&mut self, node: *mut Entry<K, V>) {
        unsafe {
            (*(*node).prev).next = (*node).next;
            (*(*node).next).prev = (*node).prev;
        }
    }
    // Attach node to head
    fn attach(&mut self, node: *mut Entry<K, V>) {
        unsafe {
            (*node).next = (*self.head).next;
            (*node).prev = self.head;
            (*self.head).next = node;
            (*(*node).next).prev = node;
        }
    }
}

impl<K, V> Drop for SortedList<K, V> {
    fn drop(&mut self) {
        // Prevent the compiler from trying to remove
        // uninitialized fields key and val in head and tail
        unsafe {
            let head = *Box::from_raw(self.head);
            let tail = *Box::from_raw(self.tail);
            let Entry {
                key: head_key,
                val: head_val,
                ..
            } = head;
            let Entry {
                key: tail_key,
                val: tail_val,
                ..
            } = tail;
            mem::forget(head_key);
            mem::forget(head_val);
            mem::forget(tail_key);
            mem::forget(tail_val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;
    use std::time::Instant;
    #[test]
    fn test_sorted_list() {
        let mut list = SortedList::new();
        for i in 0..9 {
            list.put(i, (i, Instant::now()));
            sleep(Duration::new(1, 0));
            println!("{} {:?} ", i, list.front());
        }
        while list.len() > 0 {
            match list.last() {
                Some(node) => {
                    if node.1.elapsed().as_secs() > 4 {
                        println!("node val: {:?}", node.1);
                        list.pop();
                    } else {
                        break;
                    }
                }
                None => {
                    break;
                }
            }
        }
        println!("list len: {} ", list.len());
    }
}

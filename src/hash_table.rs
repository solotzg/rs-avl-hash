use std::marker;
use std::mem;
use std::ptr;
use avl_node::{AVLNodePtr, AVLNode, AVLNodePtrBase, AVLRoot, AVLRootPtr};
use avl_node;
use std::cmp::Ordering;
use list::{ListHeadPtr, ListHead, ListHeadPtrFn};
use std::hash::Hash;
use std::hash::BuildHasher;
use std::hash::Hasher;
use std::heap::{Heap, Alloc, Layout};
use std::cmp;

pub type HashUint = usize;

const AVL_HASH_INIT_SIZE: usize = 8;

pub struct HashNode<K> {
    pub hash_val: HashUint,
    pub key: *const K,
    pub avl_node: AVLNode,
}

pub trait HashNodePtrOperation<K> {
    fn hash_val(self) -> HashUint;
    fn set_hash_val(self, hash_val: HashUint);
    fn avl_node_ptr(self) -> AVLNodePtr;
    fn key_ptr(self) -> *const K;
    fn set_key_ptr(self, key: *const K);
}

impl <K> HashNodePtrOperation<K> for *mut HashNode<K> {
    #[inline]
    fn hash_val(self) -> HashUint {
        unsafe { (*self).hash_val }
    }
    #[inline]
    fn set_hash_val(self, hash_val: HashUint) {
        unsafe { (*self).hash_val = hash_val; }
    }
    #[inline]
    fn avl_node_ptr(self) -> AVLNodePtr {
        unsafe { &mut (*self).avl_node as AVLNodePtr }
    }
    #[inline]
    fn key_ptr(self) -> *const K {
        unsafe { (*self).key }
    }
    #[inline]
    fn set_key_ptr(self, key: *const K) {
        unsafe {(*self).key = key;}
    }
}

trait ListHeadPtrOperateHashIndex {
    fn hash_index_deref_mut(self) -> *mut HashIndex;
}

impl ListHeadPtrOperateHashIndex for *mut ListHead {
    #[inline]
    fn hash_index_deref_mut(self) -> *mut HashIndex {
        container_of!(self, HashIndex, node)
    }
}

#[derive(Copy, Clone)]
pub struct HashIndex {
    avl_root: AVLRoot,
    node: ListHead,
}

impl Default for HashIndex {
    fn default() -> Self {
        HashIndex{ avl_root: Default::default(), node: Default::default() }
    }
}

pub trait HashIndexPtrOperation {
    fn avl_root_node(self) -> AVLNodePtr;
    fn node_ptr(self) -> ListHeadPtr;
    fn set_avl_root_node(self, root: AVLNodePtr);
    fn avl_root_ptr(self) -> AVLRootPtr;
    fn avl_root_node_ptr(self) -> *mut AVLNodePtr;
}

impl HashIndexPtrOperation for *mut HashIndex {
    #[inline]
    fn avl_root_node(self) -> AVLNodePtr {
        unsafe { (*self).avl_root.node }
    }

    #[inline]
    fn node_ptr(self) -> ListHeadPtr {
        unsafe { &mut (*self).node }
    }

    #[inline]
    fn set_avl_root_node(self, root: AVLNodePtr) {
        unsafe { (*self).avl_root.node = root; }
    }

    #[inline]
    fn avl_root_ptr(self) -> AVLRootPtr {
        unsafe { &mut (*self).avl_root as AVLRootPtr }
    }

    #[inline]
    fn avl_root_node_ptr(self) -> *mut AVLNodePtr {
        unsafe { &mut (*self).avl_root.node as *mut AVLNodePtr }
    }
}

pub struct HashTable<K, V> where K: Ord + Hash {
    count: usize,
    index_size: usize,
    index_mask: usize,
    head: ListHead,
    index: *mut HashIndex,
    init: [HashIndex; AVL_HASH_INIT_SIZE],
    _marker: marker::PhantomData<(K, V)>,
}

pub trait HashNodeOperation {
    fn avl_hash_deref_mut<K>(self) -> *mut HashNode<K>;
}

impl HashNodeOperation for *mut AVLNode {
    #[inline]
    fn avl_hash_deref_mut<K>(self) -> *mut HashNode<K> {
        container_of!(self, HashNode<K>, avl_node)
    }
}

#[inline]
pub fn make_hash<T: ?Sized, S>(hash_state: &S, t: &T) -> HashUint where T: Hash, S: BuildHasher {
    let mut state = hash_state.build_hasher();
    t.hash(&mut state);
    state.finish() as HashUint
}

impl<K, V> HashTable<K, V> where K: Ord + Hash {

    pub fn get_max_node_of_single_index(&self) -> i32 {
        let mut head = self.head.next;
        let mut num = 0;
        while !self.head.is_eq_ptr(head) {
            num = cmp::max(num, head.hash_index_deref_mut().avl_root_node().get_node_num());
            head = head.next();
        }
        num
    }

    #[inline]
    pub fn pop_first_index(&mut self) -> AVLNodePtr {
        let head = self.head.next;
        if self.head.is_eq_ptr(head) {
            return ptr::null_mut()
        }
        let index = head.hash_index_deref_mut();
        let avl_node = index.avl_root_node();
        debug_assert!(avl_node.not_null());
        index.set_avl_root_node(ptr::null_mut());
        head.list_del_init();
        avl_node
    }

    fn destroy(&mut self) {
        let data_ptr = self.hash_swap(ptr::null_mut(), 0);
        if !data_ptr.is_null() {
            unsafe {Heap.dealloc(data_ptr as *mut u8, Layout::from_size_align_unchecked(
                self.index_size * mem::size_of::<HashIndex>(), mem::align_of::<HashIndex>()
            ));}
        }
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn inc_count(&mut self, cnt: usize) {
        self.count += cnt;
    }

    #[inline]
    pub fn dec_count(&mut self, cnt: usize) {
        self.count -= cnt;
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn new() -> Self {
        HashTable {
            count: 0,
            index_size: 0,
            index_mask: 0,
            head: Default::default(),
            index: ptr::null_mut(),
            init: [HashIndex::default(); AVL_HASH_INIT_SIZE],
            _marker: marker::PhantomData,
        }
    }

    #[inline]
    pub fn head_ptr(&mut self) -> ListHeadPtr {
        &mut self.head as ListHeadPtr
    }

    #[inline]
    pub fn init(&mut self) {
        self.count = 0;
        self.index_size = AVL_HASH_INIT_SIZE;
        self.index_mask = self.index_size - 1;
        self.head_ptr().list_init();
        self.index = self.init.as_mut_ptr();
        for i in 0..AVL_HASH_INIT_SIZE {
            unsafe {
                (*self.index.offset(i as isize)).avl_root.node = ptr::null_mut();
                (&mut (*self.index.offset(i as isize)).node as ListHeadPtr).list_init();
            }
        }
    }

    #[inline]
    pub fn node_first(&self) -> *mut HashNode<K> {
        let head: ListHeadPtr = self.head.next as ListHeadPtr;
        if !self.head.is_eq_ptr(head) {
            let index: *mut HashIndex = head.hash_index_deref_mut();
            let avl_node = index.avl_root_node().first_node();
            if avl_node.is_null() {
                return ptr::null_mut();
            }
            return avl_node.avl_hash_deref_mut::<K>();
        }
        return ptr::null_mut();
    }

    #[inline]
    pub fn node_last(&self) -> *mut HashNode<K> {
        let head: ListHeadPtr = self.head.prev;
        if !self.head.is_eq_ptr(head) {
            let index: *mut HashIndex = head.hash_index_deref_mut();
            let avl_node = index.avl_root_node().last_node();
            if avl_node.is_null() {
                return ptr::null_mut();
            }
            return avl_node.avl_hash_deref_mut::<K>();
        }
        return ptr::null_mut();
    }

    #[inline]
    pub fn get_hash_index(&self, hash_val: HashUint) -> *mut HashIndex {
        unsafe { self.index.offset((hash_val & self.index_mask) as isize) }
    }

    #[inline]
    pub fn node_next(&self, node: *mut HashNode<K>) -> *mut HashNode<K> {
        if node.is_null() {
            return ptr::null_mut::<HashNode<K>>();
        }
        let mut avl_node = node.avl_node_ptr().next();
        if avl_node.not_null() {
            return avl_node.avl_hash_deref_mut::<K>();
        }
        let mut index = unsafe { self.get_hash_index((*node).hash_val) };
        let list_node = index.node_ptr().next();
        if self.head.is_eq_ptr(list_node) {
            return ptr::null_mut::<HashNode<K>>();
        }
        index = list_node.hash_index_deref_mut();
        avl_node = index.avl_root_node().first_node();
        if avl_node.is_null() {
            return ptr::null_mut::<HashNode<K>>();
        }
        return avl_node.avl_hash_deref_mut::<K>();
    }

    #[inline]
    pub fn node_prev(&self, node: *mut HashNode<K>) -> *mut HashNode<K> {
        if node.is_null() {
            return ptr::null_mut::<HashNode<K>>();
        }
        let mut avl_node = node.avl_node_ptr().prev();
        if avl_node.not_null() {
            return avl_node.avl_hash_deref_mut::<K>();
        }
        let mut index = unsafe { self.get_hash_index((*node).hash_val) };
        let list_node = index.node_ptr().prev();
        if self.head.is_eq_ptr(list_node) {
            return ptr::null_mut::<HashNode<K>>();
        }
        index = list_node.hash_index_deref_mut();
        avl_node = index.avl_root_node().last_node();
        if avl_node.is_null() {
            return ptr::null_mut::<HashNode<K>>();
        }
        return avl_node.avl_hash_deref_mut::<K>();
    }

    #[inline]
    pub fn hash_find(&self, hash_val: HashUint, key_ptr: *const K) -> *mut HashNode<K> {
        let index = self.get_hash_index(hash_val);
        let mut avl_node = index.avl_root_node();
        while avl_node.not_null() {
            let snode = avl_node.avl_hash_deref_mut::<K>();
            let shash_val = snode.hash_val();
            if hash_val == shash_val {
                match unsafe { (*key_ptr).cmp(&(*snode.key_ptr())) } {
                    Ordering::Greater => { avl_node = avl_node.right(); }
                    Ordering::Equal => { return snode; }
                    Ordering::Less => { avl_node = avl_node.left(); }
                }
            } else {
                avl_node = if hash_val < shash_val { avl_node.left() } else { avl_node.right() }
            }
        }
        ptr::null_mut::<HashNode<K>>()
    }

    #[inline]
    pub fn hash_erase(&mut self, node: *mut HashNode<K>) {
        debug_assert!(!node.avl_node_ptr().empty());
        let index = self.get_hash_index(node.hash_val());
        if index.avl_root_node() == node.avl_node_ptr() && node.avl_node_ptr().height() == 1 {
            index.set_avl_root_node(ptr::null_mut());
            index.node_ptr().list_del_init();
        } else {
            unsafe { avl_node::erase_node(node.avl_node_ptr(), index.avl_root_ptr()); }
        }
        node.avl_node_ptr().init();
        self.count -= 1;
    }

    #[inline]
    unsafe fn hash_track(&self, node: *mut HashNode<K>, parent: *mut AVLNodePtr) -> *mut AVLNodePtr {
        let hash = node.hash_val();
        let key = node.key_ptr();
        let index = self.get_hash_index(hash);
        let mut link = &mut index.avl_root_node() as *mut AVLNodePtr;
        (*parent) = ptr::null_mut();
        let mut tmp_node = ptr::null_mut();
        while (*link).not_null() {
            tmp_node = *link;
            let snode = tmp_node.avl_hash_deref_mut::<K>();
            let shash = snode.hash_val();
            if shash == hash {
                match (*key).cmp(&(*snode.key_ptr())) {
                    Ordering::Equal => {
                        *parent = tmp_node;
                        return ptr::null_mut();
                    }
                    Ordering::Less => {
                        link = tmp_node.left_mut();
                    }
                    Ordering::Greater => {
                        link = tmp_node.right_mut();
                    }
                }
            } else {
                link = if hash < shash { tmp_node.left_mut() } else { tmp_node.right_mut() }
            }
        }
        *parent = tmp_node;
        link
    }

    fn hash_add(&mut self, node: *mut HashNode<K>) -> *mut HashNode<K> {
        let index = self.get_hash_index(node.hash_val());
        if index.avl_root_node().is_null() {
            let tmp_node = node.avl_node_ptr();
            index.set_avl_root_node(tmp_node);
            tmp_node.reset(ptr::null_mut(),ptr::null_mut(), ptr::null_mut(), 1);
            self.head_ptr().list_add_tail(index.node_ptr());
        } else {
            let mut parent = ptr::null_mut::<AVLNode>();
            let link = unsafe { self.hash_track(node, &mut parent as *mut AVLNodePtr) };
            if link.is_null() {
                return parent.avl_hash_deref_mut::<K>();
            }
            unsafe { avl_node::link_node(node.avl_node_ptr(), parent, link); }
            unsafe { avl_node::node_post_insert(node.avl_node_ptr(), index.avl_root_ptr()); }
        }
        self.count += 1;
        ptr::null_mut()
    }

    #[inline]
    fn hash_replace(&mut self, tar: *mut HashNode<K>, new_node: *mut HashNode<K>) {
        let index = self.get_hash_index(tar.hash_val());
        unsafe { avl_node::avl_node_replace(tar.avl_node_ptr(), new_node.avl_node_ptr(), index.avl_root_ptr()); }
    }

    pub fn hash_swap(&mut self, mut new_index: *mut HashIndex, nbytes: usize) -> *mut HashIndex {
        let old_index = self.index;
        let mut index_size = 1;
        let mut head = ListHead::default();
        let head_ptr = &mut head as ListHeadPtr;
        if new_index.is_null() {
            if self.index == self.init.as_mut_ptr() {
                return ptr::null_mut();
            }
            new_index = self.init.as_mut_ptr();
            index_size = self.init.len();
        } else if new_index == old_index {
            return old_index;
        }
        if new_index != self.init.as_mut_ptr() {
            let mut test_size = mem::size_of::<HashIndex>();
            while test_size < nbytes {
                let next_size = test_size * 2;
                if next_size > nbytes {
                    break;
                }
                test_size = next_size;
                index_size = index_size * 2;
            }
        }
        self.index = new_index;
        self.index_size = index_size;
        self.index_mask = self.index_size - 1;
        self.count = 0;
        for i in 0..index_size as isize {
            unsafe { self.index.offset(i).set_avl_root_node(ptr::null_mut()); }
            unsafe { self.index.offset(i).node_ptr().list_init(); }
        }
        ListHeadPtr::list_replace(self.head_ptr(), head_ptr);
        self.head_ptr().list_init();
        while !head_ptr.list_is_empty() {
            let index = head.next.hash_index_deref_mut();
            self.recursive_hash_add(index.avl_root_node());
            index.node_ptr().list_del_init();
        }
        return if old_index == self.init.as_mut_ptr() { ptr::null_mut() } else { old_index };
    }

    fn recursive_hash_add(&mut self, node: AVLNodePtr) {
        if node.left().not_null() {
            self.recursive_hash_add(node.left());
        }
        if node.right().not_null() {
            self.recursive_hash_add(node.right());
        }
        let snode = node.avl_hash_deref_mut::<K>();
        self.hash_add(snode);
    }

    #[inline]
    pub fn rehash(&mut self, capacity: usize) {
        let index_size = self.index_size;
        let limit = (capacity * 6) / 4;
        if index_size < limit {
            let mut need = index_size;
            while need < limit {
                need *= 2;
            }
            let new_size = need * mem::size_of::<HashIndex>();
            let buffer = unsafe {Heap.alloc(Layout::from_size_align_unchecked(
                new_size, mem::align_of::<HashIndex>()
            ))}.unwrap_or_else(|e| Heap.oom(e));
            let data_ptr = self.hash_swap(buffer as *mut HashIndex, new_size);
            if !data_ptr.is_null() {
                unsafe {Heap.dealloc(data_ptr as *mut u8, Layout::from_size_align_unchecked(
                    index_size * mem::size_of::<HashIndex>(), mem::align_of::<HashIndex>()
                ));}
            }
        }
    }
}

impl <K, V> Drop for HashTable<K, V> where K: Ord + Hash {
    fn drop(&mut self) {
        self.destroy();
    }
}

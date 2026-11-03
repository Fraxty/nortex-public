//! NORTEX Standard Library: `core::memory`
//!
//! Production-grade Memory Subsystem:
//! - Global Allocator Interface (`GlobalAlloc`) & Win32 Process Heap Allocator
//! - Bump-pointer Arena Allocator with 64-bit alignment and chunk chaining
//! - High-Performance SwissTable (Group of 16 control bytes, 7-bit H2 fingerprint matching)
//! - RAII Smart Pointers: `Box<T>`, `Rc<T>` (single-thread), and `Arc<T>` (atomic multi-thread)

use std::alloc::Layout;
use std::cell::Cell;
use std::hash::{BuildHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};
use std::sync::atomic::{self, AtomicUsize, Ordering};

// ----------------------------------------------------------------------------
// 1. ALLOCATOR-SCHNITTSTELLEN & WIN32 HEAP ALLOCATOR
// ----------------------------------------------------------------------------

pub unsafe trait GlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8;
}

#[cfg(windows)]
extern "system" {
    fn GetProcessHeap() -> *mut std::ffi::c_void;
    fn HeapAlloc(hHeap: *mut std::ffi::c_void, dwFlags: u32, dwBytes: usize) -> *mut u8;
    fn HeapReAlloc(hHeap: *mut std::ffi::c_void, dwFlags: u32, lpMem: *mut u8, dwBytes: usize) -> *mut u8;
    fn HeapFree(hHeap: *mut std::ffi::c_void, dwFlags: u32, lpMem: *mut u8) -> i32;
}

pub struct Win32HeapAllocator;

unsafe impl GlobalAlloc for Win32HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[cfg(windows)]
        {
            let heap = GetProcessHeap();
            if heap.is_null() {
                return ptr::null_mut();
            }
            // Win32 Heap garantiert 16-Byte-Alignment auf x64.
            // Falls größeres Alignment benötigt wird, kann over-alignment angewendet werden.
            let size = layout.size().max(1);
            HeapAlloc(heap, 0, size)
        }
        #[cfg(not(windows))]
        {
            std::alloc::alloc(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        #[cfg(windows)]
        {
            let heap = GetProcessHeap();
            if !heap.is_null() {
                HeapFree(heap, 0, ptr);
            }
        }
        #[cfg(not(windows))]
        {
            std::alloc::dealloc(ptr, _layout);
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr.is_null() {
            return self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
        }
        #[cfg(windows)]
        {
            let heap = GetProcessHeap();
            if heap.is_null() {
                return ptr::null_mut();
            }
            HeapReAlloc(heap, 0, ptr, new_size.max(1))
        }
        #[cfg(not(windows))]
        {
            std::alloc::realloc(ptr, layout, new_size)
        }
    }
}

// ----------------------------------------------------------------------------
// 2. ARENA ALLOCATOR (BUMP ALLOCATOR MIT CHUNKS)
// ----------------------------------------------------------------------------

struct ArenaChunk {
    storage: NonNull<u8>,
    capacity: usize,
    offset: Cell<usize>,
    next: Option<Box<ArenaChunk>>,
}

impl ArenaChunk {
    fn new(capacity: usize) -> Self {
        let layout = Layout::from_size_align(capacity, 16).expect("Ungültiges Chunk-Layout");
        let ptr = unsafe { std::alloc::alloc(layout) };
        let storage = NonNull::new(ptr).unwrap_or_else(|| std::alloc::handle_alloc_error(layout));
        Self {
            storage,
            capacity,
            offset: Cell::new(0),
            next: None,
        }
    }
}

impl Drop for ArenaChunk {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity, 16).unwrap();
        unsafe {
            std::alloc::dealloc(self.storage.as_ptr(), layout);
        }
    }
}

pub struct ArenaAllocator {
    current: Cell<Option<NonNull<ArenaChunk>>>,
    head: Option<Box<ArenaChunk>>,
    default_chunk_size: usize,
}

impl ArenaAllocator {
    pub fn new(default_chunk_size: usize) -> Self {
        let mut first = Box::new(ArenaChunk::new(default_chunk_size.max(4096)));
        let raw = NonNull::new(&mut *first as *mut ArenaChunk);
        Self {
            current: Cell::new(raw),
            head: Some(first),
            default_chunk_size: default_chunk_size.max(4096),
        }
    }

    pub fn alloc<T>(&self, value: T) -> &mut T {
        let layout = Layout::new::<T>();
        let ptr = self.alloc_layout(layout) as *mut T;
        unsafe {
            ptr.write(value);
            &mut *ptr
        }
    }

    pub fn alloc_layout(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(mem::align_of::<usize>());
        let size = layout.size();

        unsafe {
            let mut curr_ptr = self.current.get().expect("Arena hat keinen aktiven Chunk");
            let chunk = curr_ptr.as_mut();

            let current_offset = chunk.offset.get();
            let aligned_offset = (current_offset + align - 1) & !(align - 1);
            let next_offset = aligned_offset + size;

            if next_offset <= chunk.capacity {
                chunk.offset.set(next_offset);
                return chunk.storage.as_ptr().add(aligned_offset);
            }

            // Neuer Chunk erforderlich
            let new_cap = self.default_chunk_size.max(size + align);
            let mut new_chunk = Box::new(ArenaChunk::new(new_cap));
            let new_ptr = NonNull::new(&mut *new_chunk as *mut ArenaChunk).unwrap();

            let chunk_aligned = (align - 1) & !(align - 1);
            new_chunk.offset.set(chunk_aligned + size);
            let allocated = new_chunk.storage.as_ptr().add(chunk_aligned);

            // Chunk in Liste einhängen
            new_chunk.next = chunk.next.take();
            chunk.next = Some(new_chunk);
            self.current.set(Some(new_ptr));

            allocated
        }
    }

    pub fn reset(&mut self) {
        let mut curr = self.head.as_mut();
        while let Some(chunk) = curr {
            chunk.offset.set(0);
            curr = chunk.next.as_mut();
        }
        if let Some(head) = self.head.as_mut() {
            self.current.set(NonNull::new(&mut **head as *mut ArenaChunk));
        }
    }
}

// ----------------------------------------------------------------------------
// 3. SWISSTABLE HASH MAP (16-BYTE GROUP SIMD-METADATEN)
// ----------------------------------------------------------------------------

const EMPTY: i8 = -128; // 0b10000000
const DELETED: i8 = -2; // 0b11111110
const GROUP_WIDTH: usize = 16;

#[inline(always)]
fn h1(hash: u64) -> usize {
    (hash >> 7) as usize
}

#[inline(always)]
fn h2(hash: u64) -> i8 {
    (hash & 0x7F) as i8
}

pub struct SwissTable<K, V, S = std::collections::hash_map::RandomState> {
    ctrl: NonNull<i8>,
    slots: NonNull<(K, V)>,
    capacity: usize, // Anzahl Slots (Potenz von 2)
    growth_left: usize,
    items: usize,
    hasher: S,
    _marker: PhantomData<(K, V)>,
}

impl<K: Eq + Hash, V> SwissTable<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(16)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(GROUP_WIDTH).next_power_of_two();
        let ctrl_layout = Layout::array::<i8>(cap + GROUP_WIDTH).unwrap();
        let slots_layout = Layout::array::<(K, V)>(cap).unwrap();

        let ctrl_ptr = unsafe {
            let p = std::alloc::alloc(ctrl_layout) as *mut i8;
            ptr::write_bytes(p, EMPTY as u8, cap + GROUP_WIDTH);
            NonNull::new_unchecked(p)
        };

        let slots_ptr = unsafe {
            let p = std::alloc::alloc(slots_layout) as *mut (K, V);
            NonNull::new_unchecked(p)
        };

        Self {
            ctrl: ctrl_ptr,
            slots: slots_ptr,
            capacity: cap,
            growth_left: (cap * 7) / 8, // 87.5% Load-Factor
            items: 0,
            hasher: std::collections::hash_map::RandomState::new(),
            _marker: PhantomData,
        }
    }

    fn hash_key<Q>(&self, key: &Q) -> u64
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + ?Sized,
    {
        let mut h = self.hasher.build_hasher();
        key.hash(&mut h);
        h.finish()
    }

    pub fn len(&self) -> usize {
        self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items == 0
    }

    #[inline(always)]
    unsafe fn match_group_16(&self, group_start: usize, target_h2: i8) -> u16 {
        // Bitmask-Matching über 16 Control-Bytes (in reiner Software äquivalent zu _mm_cmpeq_epi8)
        let mut mask: u16 = 0;
        let base = self.ctrl.as_ptr().add(group_start);
        for i in 0..16 {
            if *base.add(i) == target_h2 {
                mask |= 1 << i;
            }
        }
        mask
    }

    #[inline(always)]
    unsafe fn empty_group_16(&self, group_start: usize) -> u16 {
        let mut mask: u16 = 0;
        let base = self.ctrl.as_ptr().add(group_start);
        for i in 0..16 {
            if *base.add(i) == EMPTY {
                mask |= 1 << i;
            }
        }
        mask
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        if self.items == 0 {
            return None;
        }
        let hash = self.hash_key(key);
        let target_h2 = h2(hash);
        let mask = self.capacity - 1;
        let mut pos = h1(hash) & mask;

        unsafe {
            loop {
                let matches = self.match_group_16(pos, target_h2);
                let mut bitset = matches;
                while bitset != 0 {
                    let bit = bitset.trailing_zeros() as usize;
                    let slot_idx = (pos + bit) & mask;
                    let slot = &*self.slots.as_ptr().add(slot_idx);
                    if slot.0.borrow() == key {
                        return Some(&slot.1);
                    }
                    bitset &= bitset - 1;
                }

                // Abbruchbedingung: Wenn Gruppe mindestens einen EMPTY Slot hat, war der Schlüssel nie hier
                let empties = self.empty_group_16(pos);
                if empties != 0 {
                    return None;
                }

                pos = (pos + GROUP_WIDTH) & mask;
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.growth_left == 0 {
            self.grow();
        }

        let hash = self.hash_key(&key);
        let target_h2 = h2(hash);
        let mask = self.capacity - 1;
        let mut pos = h1(hash) & mask;

        unsafe {
            // Schritt 1: Bestehenden Key suchen
            loop {
                let matches = self.match_group_16(pos, target_h2);
                let mut bitset = matches;
                while bitset != 0 {
                    let bit = bitset.trailing_zeros() as usize;
                    let slot_idx = (pos + bit) & mask;
                    let slot = &mut *self.slots.as_ptr().add(slot_idx);
                    if slot.0 == key {
                        let old = mem::replace(&mut slot.1, value);
                        return Some(old);
                    }
                    bitset &= bitset - 1;
                }
                if self.empty_group_16(pos) != 0 {
                    break;
                }
                pos = (pos + GROUP_WIDTH) & mask;
            }

            // Schritt 2: Freien Slot (EMPTY oder DELETED) für neuen Eintrag belegen
            let mut insert_pos = h1(hash) & mask;
            loop {
                let base = self.ctrl.as_ptr().add(insert_pos);
                for i in 0..GROUP_WIDTH {
                    let ctrl = *base.add(i);
                    if ctrl == EMPTY || ctrl == DELETED {
                        let slot_idx = (insert_pos + i) & mask;
                        self.set_ctrl(slot_idx, target_h2);
                        ptr::write(self.slots.as_ptr().add(slot_idx), (key, value));
                        self.items += 1;
                        self.growth_left -= 1;
                        return None;
                    }
                }
                insert_pos = (insert_pos + GROUP_WIDTH) & mask;
            }
        }
    }

    unsafe fn set_ctrl(&mut self, index: usize, h2_val: i8) {
        *self.ctrl.as_ptr().add(index) = h2_val;
        // Spiegelung für Group-Lookups am Rand
        if index < GROUP_WIDTH {
            *self.ctrl.as_ptr().add(self.capacity + index) = h2_val;
        }
    }

    fn grow(&mut self) {
        let old_cap = self.capacity;
        let old_ctrl = self.ctrl;
        let old_slots = self.slots;

        let new_cap = old_cap * 2;
        let mut new_table = Self::with_capacity(new_cap);

        unsafe {
            for i in 0..old_cap {
                let ctrl = *old_ctrl.as_ptr().add(i);
                if ctrl != EMPTY && ctrl != DELETED {
                    let (k, v) = ptr::read(old_slots.as_ptr().add(i));
                    new_table.insert(k, v);
                }
            }

            let ctrl_layout = Layout::array::<i8>(old_cap + GROUP_WIDTH).unwrap();
            let slots_layout = Layout::array::<(K, V)>(old_cap).unwrap();
            std::alloc::dealloc(old_ctrl.as_ptr() as *mut u8, ctrl_layout);
            std::alloc::dealloc(old_slots.as_ptr() as *mut u8, slots_layout);

            self.ctrl = new_table.ctrl;
            self.slots = new_table.slots;
            self.capacity = new_table.capacity;
            self.growth_left = new_table.growth_left;
            self.items = new_table.items;

            mem::forget(new_table);
        }
    }
}

impl<K, V, S> Drop for SwissTable<K, V, S> {
    fn drop(&mut self) {
        unsafe {
            for i in 0..self.capacity {
                let ctrl = *self.ctrl.as_ptr().add(i);
                if ctrl != EMPTY && ctrl != DELETED {
                    ptr::drop_in_place(self.slots.as_ptr().add(i));
                }
            }
            let ctrl_layout = Layout::array::<i8>(self.capacity + GROUP_WIDTH).unwrap();
            let slots_layout = Layout::array::<(K, V)>(self.capacity).unwrap();
            std::alloc::dealloc(self.ctrl.as_ptr() as *mut u8, ctrl_layout);
            std::alloc::dealloc(self.slots.as_ptr() as *mut u8, slots_layout);
        }
    }
}

// ----------------------------------------------------------------------------
// 4. RAII SMART POINTER: BOX, RC, ARC
// ----------------------------------------------------------------------------

pub struct Box<T: ?Sized> {
    ptr: NonNull<T>,
    _marker: PhantomData<T>,
}

impl<T> Box<T> {
    pub fn new(value: T) -> Self {
        let layout = Layout::new::<T>();
        unsafe {
            let raw = std::alloc::alloc(layout) as *mut T;
            if raw.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            ptr::write(raw, value);
            Self {
                ptr: NonNull::new_unchecked(raw),
                _marker: PhantomData,
            }
        }
    }
}

impl<T: ?Sized> Deref for Box<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: ?Sized> Drop for Box<T> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.ptr.as_ptr());
            let layout = Layout::for_value_raw(self.ptr.as_ptr());
            std::alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
        }
    }
}

// Thread-lokaler Referenzzähler (Rc)
struct RcBox<T> {
    strong: Cell<usize>,
    value: T,
}

pub struct Rc<T> {
    ptr: NonNull<RcBox<T>>,
    _marker: PhantomData<RcBox<T>>,
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Self {
        let boxed = Box::new(RcBox {
            strong: Cell::new(1),
            value,
        });
        Self {
            ptr: NonNull::new(std::boxed::Box::into_raw(std::boxed::Box::new(*boxed))).unwrap(),
            _marker: PhantomData,
        }
    }

    pub fn strong_count(this: &Self) -> usize {
        unsafe { this.ptr.as_ref().strong.get() }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        unsafe {
            let count = self.ptr.as_ref().strong.get();
            self.ptr.as_ref().strong.set(count + 1);
        }
        Self {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &self.ptr.as_ref().value }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        unsafe {
            let count = self.ptr.as_ref().strong.get();
            if count == 1 {
                ptr::drop_in_place(&mut self.ptr.as_mut().value);
                let layout = Layout::new::<RcBox<T>>();
                std::alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
            } else {
                self.ptr.as_ref().strong.set(count - 1);
            }
        }
    }
}

// Atomarer threadsicherer Referenzzähler (Arc)
struct ArcInner<T> {
    strong: AtomicUsize,
    value: T,
}

pub struct Arc<T> {
    ptr: NonNull<ArcInner<T>>,
    _marker: PhantomData<ArcInner<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(value: T) -> Self {
        let layout = Layout::new::<ArcInner<T>>();
        unsafe {
            let raw = std::alloc::alloc(layout) as *mut ArcInner<T>;
            if raw.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            ptr::write(
                raw,
                ArcInner {
                    strong: AtomicUsize::new(1),
                    value,
                },
            );
            Self {
                ptr: NonNull::new_unchecked(raw),
                _marker: PhantomData,
            }
        }
    }

    pub fn strong_count(this: &Self) -> usize {
        unsafe { this.ptr.as_ref().strong.load(Ordering::Acquire) }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.ptr.as_ref().strong.fetch_add(1, Ordering::Relaxed);
        }
        Self {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &self.ptr.as_ref().value }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        unsafe {
            if self.ptr.as_ref().strong.fetch_sub(1, Ordering::Release) == 1 {
                atomic::fence(Ordering::Acquire);
                ptr::drop_in_place(&mut self.ptr.as_mut().value);
                let layout = Layout::new::<ArcInner<T>>();
                std::alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

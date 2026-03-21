//! 垃圾回收器模組。
//!
//! 這裡用 mark-and-sweep 管理 tiny-lang 的堆上物件。

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::environment::{CompiledFunction, Value};

pub type GcStringRef = GcRef<String>;
pub type GcArrayRef = GcRef<Vec<Value>>;
pub type GcMapRef = GcRef<HashMap<String, Value>>;
pub type GcStructRef = GcRef<StructInstanceObject>;
pub type GcClosureRef = GcRef<ClosureObject>;
pub type GcEnumVariantRef = GcRef<EnumVariantObject>;

/// GC 堆上 closure 物件。
#[derive(Debug, Clone)]
pub struct ClosureObject {
    pub function: Rc<CompiledFunction>,
    pub upvalues: Vec<Rc<RefCell<Value>>>,
}

/// GC 堆上的 struct instance。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructInstanceObject {
    pub type_name: String,
    pub fields: HashMap<String, Value>,
}

/// GC 堆上的 enum variant。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantObject {
    pub enum_name: String,
    pub variant_name: String,
    pub fields: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
enum HeapObject {
    String(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Closure(ClosureObject),
    StructInstance(StructInstanceObject),
    EnumVariant(EnumVariantObject),
}

#[derive(Debug, Clone)]
struct HeapEntry {
    marked: bool,
    object: Option<HeapObject>,
}

#[derive(Debug, Default)]
struct HeapStorage {
    objects: Vec<HeapEntry>,
}

/// GC 管理的引用，底層用 index 指向堆物件。
#[derive(Debug)]
pub struct GcRef<T> {
    pub(crate) index: usize,
    storage: Rc<RefCell<HeapStorage>>,
    _marker: PhantomData<T>,
}

impl<T> Clone for GcRef<T> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            storage: self.storage.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> PartialEq for GcRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Rc::ptr_eq(&self.storage, &other.storage)
    }
}

impl<T> Eq for GcRef<T> {}

/// GC 統計資訊。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcStats {
    pub total_allocations: usize,
    pub total_collections: usize,
    pub current_heap_size: usize,
}

/// 垃圾回收堆。
#[derive(Debug, Clone)]
pub struct GcHeap {
    storage: Rc<RefCell<HeapStorage>>,
    pub total_allocations: usize,
    pub total_collections: usize,
    next_gc_threshold: usize,
}

impl Default for GcHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl GcHeap {
    pub fn new() -> Self {
        Self {
            storage: Rc::new(RefCell::new(HeapStorage::default())),
            total_allocations: 0,
            total_collections: 0,
            next_gc_threshold: 1024,
        }
    }

    pub fn stats(&self) -> GcStats {
        GcStats {
            total_allocations: self.total_allocations,
            total_collections: self.total_collections,
            current_heap_size: self.current_heap_size(),
        }
    }

    pub fn current_heap_size(&self) -> usize {
        self.storage
            .borrow()
            .objects
            .iter()
            .filter(|entry| entry.object.is_some())
            .count()
    }

    pub fn should_collect(&self) -> bool {
        self.current_heap_size() >= self.next_gc_threshold
    }

    pub fn alloc_string(&mut self, value: String) -> GcStringRef {
        self.alloc_object(HeapObject::String(value))
    }

    pub fn alloc_array(&mut self, value: Vec<Value>) -> GcArrayRef {
        self.alloc_object(HeapObject::Array(value))
    }

    pub fn alloc_map(&mut self, value: HashMap<String, Value>) -> GcMapRef {
        self.alloc_object(HeapObject::Map(value))
    }

    pub fn alloc_closure(&mut self, value: ClosureObject) -> GcClosureRef {
        self.alloc_object(HeapObject::Closure(value))
    }

    pub fn alloc_struct_instance(&mut self, value: StructInstanceObject) -> GcStructRef {
        self.alloc_object(HeapObject::StructInstance(value))
    }

    pub fn alloc_enum_variant(&mut self, value: EnumVariantObject) -> GcEnumVariantRef {
        self.alloc_object(HeapObject::EnumVariant(value))
    }

    fn alloc_object<T>(&mut self, object: HeapObject) -> GcRef<T> {
        // 先嘗試重用已被 GC 釋放（object == None）的 slot，避免陣列無限增長
        let index = {
            let mut storage = self.storage.borrow_mut();
            if let Some(free_index) = storage.objects.iter().position(|e| e.object.is_none()) {
                // 重用已釋放的 slot
                storage.objects[free_index] = HeapEntry {
                    marked: false,
                    object: Some(object),
                };
                free_index
            } else {
                // 沒有空閒 slot，在陣列末尾追加新物件
                let index = storage.objects.len();
                storage.objects.push(HeapEntry {
                    marked: false,
                    object: Some(object),
                });
                index
            }
        };
        self.total_allocations += 1;
        GcRef {
            index,
            storage: self.storage.clone(),
            _marker: PhantomData,
        }
    }

    pub fn get_string(&self, reference: &GcStringRef) -> String {
        match self.get_object(reference.index) {
            HeapObject::String(value) => value,
            _ => panic!("GC type mismatch: expected String"),
        }
    }

    pub fn with_array<R>(&self, reference: &GcArrayRef, f: impl FnOnce(&Vec<Value>) -> R) -> R {
        let storage = self.storage.borrow();
        let entry = storage
            .objects
            .get(reference.index)
            .and_then(|entry| entry.object.as_ref())
            .expect("dangling GC reference");
        match entry {
            HeapObject::Array(value) => f(value),
            _ => panic!("GC type mismatch: expected Array"),
        }
    }

    pub fn with_array_mut<R>(&mut self, reference: &GcArrayRef, f: impl FnOnce(&mut Vec<Value>) -> R) -> R {
        let mut storage = self.storage.borrow_mut();
        let entry = storage
            .objects
            .get_mut(reference.index)
            .and_then(|entry| entry.object.as_mut())
            .expect("dangling GC reference");
        match entry {
            HeapObject::Array(value) => f(value),
            _ => panic!("GC type mismatch: expected Array"),
        }
    }

    pub fn with_map<R>(&self, reference: &GcMapRef, f: impl FnOnce(&HashMap<String, Value>) -> R) -> R {
        let storage = self.storage.borrow();
        let entry = storage
            .objects
            .get(reference.index)
            .and_then(|entry| entry.object.as_ref())
            .expect("dangling GC reference");
        match entry {
            HeapObject::Map(value) => f(value),
            _ => panic!("GC type mismatch: expected Map"),
        }
    }

    pub fn with_map_mut<R>(
        &mut self,
        reference: &GcMapRef,
        f: impl FnOnce(&mut HashMap<String, Value>) -> R,
    ) -> R {
        let mut storage = self.storage.borrow_mut();
        let entry = storage
            .objects
            .get_mut(reference.index)
            .and_then(|entry| entry.object.as_mut())
            .expect("dangling GC reference");
        match entry {
            HeapObject::Map(value) => f(value),
            _ => panic!("GC type mismatch: expected Map"),
        }
    }

    pub fn get_struct_instance(&self, reference: &GcStructRef) -> StructInstanceObject {
        match self.get_object(reference.index) {
            HeapObject::StructInstance(value) => value,
            _ => panic!("GC type mismatch: expected StructInstance"),
        }
    }

    pub fn with_struct_instance<R>(
        &self,
        reference: &GcStructRef,
        f: impl FnOnce(&StructInstanceObject) -> R,
    ) -> R {
        let storage = self.storage.borrow();
        let entry = storage
            .objects
            .get(reference.index)
            .and_then(|entry| entry.object.as_ref())
            .expect("dangling GC reference");
        match entry {
            HeapObject::StructInstance(value) => f(value),
            _ => panic!("GC type mismatch: expected StructInstance"),
        }
    }

    pub fn with_struct_instance_mut<R>(
        &mut self,
        reference: &GcStructRef,
        f: impl FnOnce(&mut StructInstanceObject) -> R,
    ) -> R {
        let mut storage = self.storage.borrow_mut();
        let entry = storage
            .objects
            .get_mut(reference.index)
            .and_then(|entry| entry.object.as_mut())
            .expect("dangling GC reference");
        match entry {
            HeapObject::StructInstance(value) => f(value),
            _ => panic!("GC type mismatch: expected StructInstance"),
        }
    }

    pub fn get_enum_variant(&self, reference: &GcEnumVariantRef) -> EnumVariantObject {
        match self.get_object(reference.index) {
            HeapObject::EnumVariant(value) => value,
            _ => panic!("GC type mismatch: expected EnumVariant"),
        }
    }

    pub fn with_enum_variant<R>(
        &self,
        reference: &GcEnumVariantRef,
        f: impl FnOnce(&EnumVariantObject) -> R,
    ) -> R {
        let storage = self.storage.borrow();
        let entry = storage
            .objects
            .get(reference.index)
            .and_then(|entry| entry.object.as_ref())
            .expect("dangling GC reference");
        match entry {
            HeapObject::EnumVariant(value) => f(value),
            _ => panic!("GC type mismatch: expected EnumVariant"),
        }
    }

    pub fn get_closure(&self, reference: &GcClosureRef) -> ClosureObject {
        match self.get_object(reference.index) {
            HeapObject::Closure(value) => value,
            _ => panic!("GC type mismatch: expected Closure"),
        }
    }

    pub fn mark_and_sweep(&mut self, roots: &[Value], constant_roots: &[Value]) {
        for value in roots {
            self.mark_value(value);
        }
        for value in constant_roots {
            self.mark_value(value);
        }

        {
            let mut storage = self.storage.borrow_mut();
            for entry in &mut storage.objects {
                if entry.object.is_none() {
                    continue;
                }
                if entry.marked {
                    entry.marked = false;
                } else {
                    entry.object = None;
                }
            }
        }

        self.total_collections += 1;
        self.next_gc_threshold = self.current_heap_size().max(1024) * 2;
    }

    pub fn mark_value(&mut self, value: &Value) {
        match value {
            Value::String(reference) => self.mark_index(reference.index),
            Value::Array(reference) => self.mark_index(reference.index),
            Value::Map(reference) => self.mark_index(reference.index),
            Value::Closure(reference) => self.mark_index(reference.index),
            Value::StructInstance(reference) => self.mark_index(reference.index),
            Value::EnumVariant(reference) => self.mark_index(reference.index),
            _ => {}
        }
    }

    fn mark_index(&mut self, index: usize) {
        let object = {
            let mut storage = self.storage.borrow_mut();
            let Some(entry) = storage.objects.get_mut(index) else {
                return;
            };
            if entry.marked || entry.object.is_none() {
                return;
            }
            entry.marked = true;
            entry.object.clone()
        };

        let Some(object) = object else {
            return;
        };

        match object {
            HeapObject::String(_) => {}
            HeapObject::Array(items) => {
                for item in items {
                    self.mark_value(&item);
                }
            }
            HeapObject::Map(items) => {
                for value in items.values() {
                    self.mark_value(value);
                }
            }
            HeapObject::Closure(closure) => {
                for value in &closure.function.chunk.constants {
                    self.mark_value(value);
                }
                for cell in closure.upvalues {
                    self.mark_value(&cell.borrow());
                }
            }
            HeapObject::StructInstance(instance) => {
                for value in instance.fields.values() {
                    self.mark_value(value);
                }
            }
            HeapObject::EnumVariant(variant) => {
                for value in variant.fields.values() {
                    self.mark_value(value);
                }
            }
        }
    }

    fn get_object(&self, index: usize) -> HeapObject {
        self.storage
            .borrow()
            .objects
            .get(index)
            .and_then(|entry| entry.object.clone())
            .expect("dangling GC reference")
    }
}

use std::any::{Any, TypeId};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 上下文 Value key 约束：可比较、可哈希、'static。
pub trait ValueKey: Send + Sync + 'static {
    fn equals(&self, other: &dyn ValueKey) -> bool;
    fn hash_dyn(&self, state: &mut dyn Hasher);
    fn type_id(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
}

impl<T> ValueKey for T
where
    T: Eq + Hash + Send + Sync + 'static,
{
    fn equals(&self, other: &dyn ValueKey) -> bool {
        if Any::type_id(self) != other.type_id() {
            return false;
        }
        other
            .as_any()
            .downcast_ref::<T>()
            .map(|v| v == self)
            .unwrap_or(false)
    }

    fn hash_dyn(&self, state: &mut dyn Hasher) {
        let mut tid_hasher = DefaultHasher::new();
        TypeId::of::<T>().hash(&mut tid_hasher);
        let mut val_hasher = DefaultHasher::new();
        Hash::hash(self, &mut val_hasher);
        state.write_u64(tid_hasher.finish());
        state.write_u64(val_hasher.finish());
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

use std::{
  borrow::Borrow,
  collections::{hash_map::RandomState, HashSet},
  hash::{Hash, BuildHasher},
  ops::Index
};

use indexmap::{IndexMap};
use smallvec::{smallvec, SmallVec};

#[derive(Clone)]
pub struct ScopeMap<K, V, S: BuildHasher = RandomState> {
  map: IndexMap<K, SmallVec<[V; 1]>, S>,
  layers: SmallVec<[HashSet<usize>; 1]>,
}

impl<K, V, S: Default + BuildHasher> Default for ScopeMap<K, V, S> {
  #[inline]
  fn default() -> Self {
    Self::with_hasher(Default::default())
  }
}

impl<K, Q: ?Sized, V, S> Index<&Q> for ScopeMap<K, V, S>
where 
  K: Eq + Hash + Borrow<Q>,
  Q: Eq + Hash,
  S: BuildHasher,
{
  type Output = V;

  /// Returns a reference to the value associated with the provided key.
  ///
  /// # Panics
  ///
  /// Panics if the key does not exist in the `ScopeMap`.
  #[inline]
  fn index(&self, index: &Q) -> &Self::Output {
    self.get(index).expect("key not found in map")
  }
}

impl<K, V> ScopeMap<K, V, RandomState> {
  #[inline]
  pub fn new() -> ScopeMap<K, V, RandomState> {
    Self {
      map: Default::default(),
      layers: smallvec![Default::default()],
    }
  }
  
  #[inline]
  pub fn with_capacity(capacity: usize) -> ScopeMap<K, V, RandomState> {
    Self::with_capacity_and_hasher(capacity, Default::default())
  }
}

impl<K, V, S: BuildHasher> ScopeMap<K, V, S> {
  #[inline]
  pub fn with_hasher(hash_builder: S) -> Self {
    Self {
      map: IndexMap::with_hasher(hash_builder),
      layers: smallvec![Default::default()],
    }
  }
  
  #[inline]
  pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
    Self {
      map: IndexMap::with_capacity_and_hasher(capacity, hash_builder),
      layers: smallvec![Default::default()],
    }
  }
  
  #[inline]
  pub fn capacity(&self) -> usize {
    self.map.capacity()
  }

  /// Returns `true` if the map has no keys.
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.map.is_empty()
  }
  
  /// Gets the number of distinct keys throughout all layers.
  #[inline]
  pub fn len(&self) -> usize {
    self.map.len()
  }
  
  /// Gets the number of active layers.
  #[inline]
  pub fn layer_count(&self) -> usize {
    self.layers.len()
  }
}

impl<K, V, S> ScopeMap<K, V, S> 
where 
  S: BuildHasher,
{
  /// Adds a new, empty layer.
  #[inline]
  pub fn push_layer(&mut self) {
    self.layers.push(Default::default())
  }
  
  /// Removes the topmost layer, if there is more than one.
  /// Returns `true` if a layer was removed.
  #[inline]
  pub fn pop_layer(&mut self) -> bool {
    if self.layers.len() > 1 {
      for stack_index in self.layers.pop().unwrap() {
        if let Some((_key, stack)) = self.map.get_index_mut(stack_index) {
          stack.pop();
        }
      }
      return true;
    }
    false
  }
}

impl<K: Eq + Hash, V, S: BuildHasher> ScopeMap<K, V, S> {
  
  /// Returns `true` if the map contains the specified key in any layer.
  #[inline]
  pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
  where
    K: Borrow<Q>,
    Q: Eq + Hash,
  {
    self.map.contains_key(key)
  } 

  /// Returns `true` if the map contains the specified key at the top layer.
  #[inline]
  pub fn contains_key_at_top<Q: ?Sized>(&self, key: &Q) -> bool
  where
    K: Borrow<Q>,
    Q: Eq + Hash,
  {
    self.map.get_full(key).map_or(false, |(index, ..)| self.layers.last().unwrap().contains(&index))
  }
  
  /// Gets a reference to the topmost value associated with a key.
  #[inline]
  pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
  where
  K: Borrow<Q>,
  Q: Eq + Hash,
  {
    self.map.get(key).and_then(|v| v.last())
  }
  
  /// Gets a mutable reference to the topmost value associated with a key.
  #[inline]
  pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
  where
  K: Borrow<Q>,
  Q: Eq + Hash,
  {
    self.map.get_mut(key).and_then(|v| v.last_mut())
  }
  
  /// Gets a reference to a value `skip_count` layers below the topmost value associated with a key.
  #[inline]
  pub fn get_parent<Q: ?Sized>(&self, key: &Q, skip_count: usize) -> Option<&V>
  where
  K: Borrow<Q>,
  Q: Eq + Hash,
  {
    if let Some((stack_index, _key, stack)) = self.map.get_full(key) {
      // If the skip count exceeds the stack size, it shouldn't matter because take() is self-truncating
      let stack_skip_count = self
      .layers
      .iter()
      .rev()
      .take(skip_count)
      .filter(|layer| layer.contains(&stack_index))
      .count();
      return stack.iter().rev().nth(stack_skip_count)
    }
    None
  }
  
  /// Gets a mutable reference to a value `skip_count` layers below the topmost value associated with a key.
  #[inline]
  pub fn get_parent_mut<Q: ?Sized>(&mut self, key: &Q, skip_count: usize) -> Option<&mut V>
  where
    K: Borrow<Q>,
    Q: Eq + Hash,
  {
    if let Some((stack_index, _key, stack)) = self.map.get_full_mut(key) {
      // If the skip count exceeds the stack size, it shouldn't matter because take() is self-truncating
      let stack_skip_count = self
      .layers
      .iter()
      .rev()
      .take(skip_count)
      .filter(|layer| layer.contains(&stack_index))
      .count();
      return stack.iter_mut().rev().nth(stack_skip_count)
    }
    None
  }
  
  /// Adds a value to the topmost layer.
  #[inline]
  pub fn define(&mut self, key: K, value: V) {
    let entry = self.map.entry(key);
    let stack_index = entry.index();
    let stack = entry.or_insert_with(Default::default);    
    let is_new = self.layers.last_mut().unwrap().insert(stack_index);
    
    if is_new {
      stack.push(value);
    } else {
      *stack.last_mut().unwrap() = value;
    }
  }
  
  /// Removes a value from the topmost layer.
  #[inline]
  pub fn delete(&mut self, key: K) -> bool {
    if let Some((index, _key, stack)) = self.map.get_full_mut(&key) {
      if self.layers.last_mut().unwrap().remove(&index) {
        stack.pop();
        return true
      }
    }
    false
  }
  
  /// Removes all entries in the topmost layer.
  #[inline]
  pub fn clear_top(&mut self) {
    for stack_index in self.layers.last_mut().unwrap().drain() {
      self.map.get_index_mut(stack_index).unwrap().1.pop();
    }
  }
  
  /// Removes all elements and additional layers.
  #[inline]
  pub fn clear_all(&mut self) {
    self.map.clear();
    self.layers.clear();
    self.layers.push(Default::default())
  }
}
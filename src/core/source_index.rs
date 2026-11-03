//! Lazily index source containers once per immutable source revision.
use serde_json::value::RawValue;
use std::{
    collections::HashMap,
    ops::Range,
    sync::{Arc, OnceLock},
};

struct Node {
    range: Range<usize>,
    bound: usize,
    key: String,
    children: OnceLock<Result<Vec<Node>, String>>,
}

pub struct SourceIndex {
    text: Arc<String>,
    root: Node,
}

impl SourceIndex {
    pub fn new(text: Arc<String>) -> Result<Self, String> {
        let raw: &RawValue = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let start = raw.get().as_ptr() as usize - text.as_ptr() as usize;
        let root = Node {
            range: start..start + raw.get().len(),
            bound: start,
            key: String::new(),
            children: OnceLock::new(),
        };
        Ok(Self { text, root })
    }

    fn children<'a>(&self, node: &'a Node) -> Result<&'a Vec<Node>, String> {
        node.children
            .get_or_init(|| {
                let raw = &self.text[node.range.clone()];
                let entries: Vec<(String, &RawValue)> = match raw.as_bytes().first() {
                    Some(b'{') => serde_json::from_str::<HashMap<String, &RawValue>>(raw)
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .collect(),
                    Some(b'[') => serde_json::from_str::<Vec<&RawValue>>(raw)
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .enumerate()
                        .map(|(i, v)| (i.to_string(), v))
                        .collect(),
                    _ => Vec::new(),
                };
                let object = raw.starts_with('{');
                let mut children: Vec<_> = entries
                    .into_iter()
                    .map(|(key, value)| {
                        let start = value.get().as_ptr() as usize - self.text.as_ptr() as usize;
                        Node {
                            range: start..start + value.get().len(),
                            bound: if object {
                                super::edit::property_start(&self.text, start).unwrap_or(start)
                            } else {
                                start
                            },
                            key,
                            children: OnceLock::new(),
                        }
                    })
                    .collect();
                children.sort_unstable_by_key(|node| node.bound);
                Ok(children)
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    pub fn pointer_at_offset(&self, offset: usize) -> Result<Option<String>, String> {
        if !self.root.range.contains(&offset) {
            return Ok(None);
        }
        let mut node = &self.root;
        let mut pointer = String::new();
        loop {
            let children = self.children(node)?;
            let index = children.partition_point(|child| child.bound <= offset);
            let Some(child) = index.checked_sub(1).and_then(|i| children.get(i)) else {
                break;
            };
            if self.text.as_bytes()[node.range.start] == b'{' {
                if offset >= node.range.end - 1 {
                    break;
                }
                crate::append_pointer(&mut pointer, &child.key);
                if offset < child.range.start || offset > child.range.end {
                    break;
                }
            } else {
                if offset > child.range.end
                    || (offset == child.range.end
                        && matches!(self.text.as_bytes().get(offset), Some(b']' | b'}')))
                {
                    break;
                }
                crate::append_pointer(&mut pointer, &child.key);
            }
            node = child;
        }
        Ok(Some(pointer))
    }

    pub fn source_range(&self, pointer: &str) -> Result<Range<usize>, String> {
        let mut node = &self.root;
        if !pointer.is_empty() {
            for token in pointer
                .strip_prefix('/')
                .ok_or("Invalid JSON Pointer")?
                .split('/')
            {
                let mut key = String::new();
                let mut chars = token.chars();
                while let Some(ch) = chars.next() {
                    key.push(if ch == '~' {
                        match chars.next() {
                            Some('0') => '~',
                            Some('1') => '/',
                            _ => return Err("Invalid pointer escape".into()),
                        }
                    } else {
                        ch
                    });
                }
                let children = self.children(node)?;
                node = match self.text.as_bytes()[node.range.start] {
                    b'{' => children
                        .iter()
                        .find(|child| child.key == key)
                        .ok_or("Pointer not found")?,
                    b'[' => {
                        if key.len() > 1 && key.starts_with('0') {
                            return Err("Invalid array index".into());
                        }
                        let index: usize = key.parse().map_err(|_| "Invalid array index")?;
                        children.get(index).ok_or("Array index out of range")?
                    }
                    _ => return Err("Cannot descend into a scalar".into()),
                };
            }
        }
        Ok(node.range.clone())
    }
}

// Keep only two source revisions per processing thread. Arc identity avoids hashing
// or comparing document contents on each navigation action.
thread_local! {
    static CACHE: std::cell::RefCell<Vec<(Arc<String>, Arc<SourceIndex>)>> = const { std::cell::RefCell::new(Vec::new()) };
}
pub fn cached(text: &Arc<String>) -> Result<Arc<SourceIndex>, String> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache
            .iter()
            .position(|(source, _)| Arc::ptr_eq(source, text))
        {
            let entry = cache.remove(index);
            let result = entry.1.clone();
            cache.push(entry);
            return Ok(result);
        }
        let index = Arc::new(SourceIndex::new(text.clone())?);
        if cache.len() == 2 {
            cache.remove(0);
        }
        cache.push((text.clone(), index.clone()));
        Ok(index)
    })
}

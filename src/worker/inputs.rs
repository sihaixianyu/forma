use serde::{Deserialize, Serialize};
use std::{cell::RefCell, sync::Arc};

// Two bounded slots in each realm: acknowledged inputs on the UI side and
// resident inputs in the Worker. IDs are never inferred from pointer addresses.
pub(super) type InputSlot = Option<(u64, Arc<String>)>;
thread_local! {
    pub(super) static INPUTS: RefCell<[InputSlot; 2]> = const { RefCell::new([None, None]) };
    static NEXT_INPUT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}
#[derive(Serialize, Deserialize)]
pub(super) struct InputRef {
    id: u64,
    text: Option<Arc<String>>,
}
impl InputRef {
    pub(super) fn new(side: usize, text: &Arc<String>, force: bool) -> Self {
        let known = INPUTS.with(|inputs| {
            inputs.borrow()[side]
                .as_ref()
                .filter(|(_, old)| Arc::ptr_eq(old, text) || old == text)
                .map(|(id, _)| *id)
        });
        let id = known.unwrap_or_else(|| {
            NEXT_INPUT.with(|next| {
                let id = next.get();
                next.set(id.checked_add(1).expect("input ID exhausted"));
                id
            })
        });
        Self {
            id,
            text: (force || known.is_none()).then(|| text.clone()),
        }
    }
    pub(super) fn resolve(self, side: usize) -> Result<Arc<String>, String> {
        INPUTS.with(|inputs| {
            let mut inputs = inputs.borrow_mut();
            if let Some((id, text)) = &inputs[side]
                && *id == self.id
            {
                return Ok(text.clone());
            }
            let text = self.text.ok_or("cache_miss")?;
            inputs[side] = Some((self.id, text.clone()));
            Ok(text)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acknowledged_inputs_omit_text_and_cache_misses_retry_with_full_text() {
        INPUTS.with(|inputs| *inputs.borrow_mut() = [None, None]);
        let left = Arc::new("{\"left\":1}".into());
        let right = Arc::new("{\"right\":1}".into());
        let first = InputRef::new(0, &left, false);
        assert!(first.text.is_some());
        assert_eq!(first.resolve(0).unwrap(), left);
        InputRef::new(1, &right, false).resolve(1).unwrap();
        let reused = InputRef::new(0, &Arc::new((*left).clone()), false);
        assert!(reused.text.is_none());
        let changed = Arc::new("{\"right\":2}".into());
        assert!(InputRef::new(1, &changed, false).text.is_some());
        // Simulate Worker cache loss after the UI has prepared a reference.
        let retry = InputRef::new(0, &left, true);
        INPUTS.with(|inputs| *inputs.borrow_mut() = [None, None]);
        assert_eq!(reused.resolve(0).unwrap_err(), "cache_miss");
        assert_eq!(retry.resolve(0).unwrap(), left);
        assert!(InputRef::new(0, &left, false).text.is_none());
    }
}

use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
};

fn fingerprint(value: &Value) -> u64 {
    fn hash(value: &Value, state: &mut DefaultHasher) {
        std::mem::discriminant(value).hash(state);
        match value {
            Value::Object(map) => {
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort_unstable();
                for key in keys {
                    key.hash(state);
                    hash(&map[key], state);
                }
            }
            Value::Array(array) => array.iter().for_each(|child| hash(child, state)),
            Value::String(text) => text.hash(state),
            _ => value.to_string().hash(state),
        }
    }
    let mut state = DefaultHasher::new();
    hash(value, &mut state);
    state.finish()
}

struct Features {
    tokens: Vec<(u64, usize)>,
    weight: usize,
}

fn features(value: &Value) -> Features {
    fn walk(value: &Value, path: &str, out: &mut Vec<u64>, remaining: &mut usize) {
        if *remaining == 0 {
            return;
        }
        *remaining -= 1;
        match value {
            Value::Object(map) => {
                // Stable traversal even when source key order differs.
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort_unstable();
                for key in keys {
                    if *remaining == 0 {
                        break;
                    }
                    let path = crate::pointer_child(path, key);
                    let mut state = DefaultHasher::new();
                    path.hash(&mut state);
                    out.push(state.finish());
                    walk(&map[key], &path, out, remaining);
                }
            }
            Value::Array(array) => {
                for (index, child) in array.iter().enumerate() {
                    if *remaining == 0 {
                        break;
                    }
                    walk(
                        child,
                        &crate::pointer_child(path, &index.to_string()),
                        out,
                        remaining,
                    );
                }
            }
            _ => {
                let mut state = DefaultHasher::new();
                path.hash(&mut state);
                fingerprint(value).hash(&mut state);
                // Equal field values matter more than shared field names.
                out.extend(std::iter::repeat_n(state.finish(), 16));
                if let Value::String(text) = value {
                    let chars: Vec<_> = text.chars().take(128).collect();
                    let step = chars.len().saturating_sub(1).div_ceil(8).max(1);
                    for pair in chars.windows(2).step_by(step).take(8) {
                        let mut state = DefaultHasher::new();
                        path.hash(&mut state);
                        "text_pair".hash(&mut state);
                        pair.hash(&mut state);
                        out.push(state.finish());
                    }
                }
            }
        }
    }
    let mut result = Vec::new();
    walk(value, "", &mut result, &mut 256);
    result.sort_unstable();
    let weight = result.len();
    let mut tokens: Vec<(u64, usize)> = Vec::new();
    for token in result {
        if let Some((previous, count)) = tokens.last_mut()
            && *previous == token
        {
            *count += 1;
        } else {
            tokens.push((token, 1));
        }
    }
    Features { tokens, weight }
}

fn similarity(a: &Features, b: &Features) -> usize {
    let total = a.weight + b.weight;
    let (a, b) = (&a.tokens, &b.tokens);
    let (mut i, mut j, mut shared) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                shared += a[i].1.min(b[j].1);
                i += 1;
                j += 1;
            }
        }
    }
    (2_000_000_u64 * shared as u64 / total.max(1) as u64) as usize
}

/// One-to-one matches in left order. Exact matches are reserved before scoring
/// changed elements; duplicates keep their multiplicity and ties stay stable.
pub(crate) fn pairs(left: &[Value], right: &[Value]) -> Vec<Option<usize>> {
    if left == right {
        return (0..left.len()).map(Some).collect();
    }
    if left.is_empty() || right.is_empty() {
        return vec![None; left.len()];
    }
    let mut available: HashMap<u64, VecDeque<usize>> = HashMap::new();
    for (index, value) in right.iter().enumerate() {
        available
            .entry(fingerprint(value))
            .or_default()
            .push_back(index);
    }
    let mut matches = vec![None; left.len()];
    let mut used = vec![false; right.len()];
    for (i, value) in left.iter().enumerate() {
        if let Some(candidates) = available.get_mut(&fingerprint(value))
            && let Some(position) = candidates.iter().position(|j| value == &right[*j])
        {
            let j = candidates.remove(position).unwrap();
            matches[i] = Some(j);
            used[j] = true;
        }
    }
    let a: Vec<_> = (0..left.len()).filter(|i| matches[*i].is_none()).collect();
    let b: Vec<_> = (0..right.len()).filter(|j| !used[*j]).collect();
    // Bound fuzzy work after the linear exact-match pass on large arrays.
    if a.len().saturating_mul(b.len()) <= 65_536 {
        let af: Vec<_> = a.iter().map(|i| features(&left[*i])).collect();
        let bf: Vec<_> = b.iter().map(|j| features(&right[*j])).collect();
        let mut candidates = Vec::new();
        for (ai, i) in a.iter().enumerate() {
            for (bi, j) in b.iter().enumerate() {
                let score = similarity(&af[ai], &bf[bi]);
                if score > 0 {
                    candidates.push((std::cmp::Reverse(score), i.abs_diff(*j), *i, *j));
                }
            }
        }
        candidates.sort_unstable();
        for (_, _, i, j) in candidates {
            if matches[i].is_none() && !used[j] {
                matches[i] = Some(j);
                used[j] = true;
            }
        }
    }
    // Unrelated replacements and budget overflow retain their relative order.
    let mut rest = b.into_iter().filter(|j| !used[*j]);
    for i in a {
        if matches[i].is_none() {
            matches[i] = rest.next();
        }
    }
    matches
}

/// A fuzzy choice is uncertain when another remaining item scores as well,
/// or when there is little shared content. Exact matches are unambiguous here.
pub(crate) fn uncertainties(left: &[Value], right: &[Value], pairs: &[Option<usize>]) -> Vec<bool> {
    if pairs
        .iter()
        .enumerate()
        .all(|(i, j)| j.is_none_or(|j| left[i] == right[j]))
    {
        return vec![false; left.len()];
    }
    let alternatives: Vec<_> = right.iter().map(features).collect();
    pairs
        .iter()
        .enumerate()
        .map(|(i, j)| {
            let Some(j) = j else {
                return false;
            };
            if left[i] == right[*j] {
                return false;
            }
            let a = features(&left[i]);
            let score = similarity(&a, &alternatives[*j]);
            score < 250_000
                || alternatives
                    .iter()
                    .filter(|b| similarity(&a, b) >= score)
                    .take(2)
                    .count()
                    > 1
        })
        .collect()
}

pub(super) fn reorder(reference: &[Value], value: &mut Vec<Value>) -> (bool, Vec<Option<usize>>) {
    let matched = pairs(reference, value);
    let mut order: Vec<_> = matched.iter().flatten().copied().collect();
    let mut used = vec![false; value.len()];
    for index in &order {
        used[*index] = true;
    }
    order.extend((0..value.len()).filter(|index| !used[*index]));
    let changed = order.iter().enumerate().any(|(i, j)| i != *j);
    let mut positions = vec![0; value.len()];
    for (new, old) in order.iter().enumerate() {
        positions[*old] = new;
    }
    if changed {
        let mut original: Vec<_> = std::mem::take(value).into_iter().map(Some).collect();
        value.extend(
            order
                .into_iter()
                .map(|index| original[index].take().unwrap()),
        );
    }
    (
        changed,
        matched
            .into_iter()
            .map(|index| index.map(|index| positions[index]))
            .collect(),
    )
}

//! CRDT commutativity and idempotency checks.
//!
//! Operations on a convergent CRDT must produce the same final state no matter
//! the delivery order (commutativity) and must be safe to re-apply
//! (idempotency). These helpers assert exactly that for a caller-provided
//! state machine and are intended for use inside `proptest!` blocks.

use std::collections::BTreeMap;
use std::fmt::Debug;

/// Asserts that applying `op_a` then `op_b` converges to the same state as
/// `op_b` then `op_a`.
///
/// ```rust
/// use tpt_av_test_fuzz::crdt::{apply_register, assert_crdt_commutative, Registers, RegisterOp};
///
/// let initial = Registers::default();
/// let set_x = RegisterOp { key: "x".into(), ts: 1, value: Some("a".into()) };
/// let set_y = RegisterOp { key: "x".into(), ts: 2, value: Some("b".into()) };
///
/// assert_crdt_commutative(initial, set_x, set_y, apply_register);
/// ```
pub fn assert_crdt_commutative<S, A, F>(initial: S, op_a: A, op_b: A, apply: F)
where
    S: Clone + PartialEq + Debug,
    A: Clone,
    F: Fn(S, A) -> S,
{
    let ab = apply(apply(initial.clone(), op_a.clone()), op_b.clone());
    let ba = apply(apply(initial, op_b), op_a);
    assert_eq!(ab, ba, "CRDT outcome depends on operation order");
}

/// Asserts that applying `op` twice produces the same state as applying it
/// once (deliveries are not deduplicated by the network layer).
pub fn assert_crdt_idempotent<S, A, F>(initial: S, op: A, apply: F)
where
    S: Clone + PartialEq + Debug,
    A: Clone,
    F: Fn(S, A) -> S,
{
    let once = apply(initial, op.clone());
    let twice = apply(once.clone(), op);
    assert_eq!(
        once, twice,
        "re-applying the same CRDT operation changed the state"
    );
}

/// A single mutation of a LWW-register key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterOp {
    /// The key being mutated.
    pub key: String,
    /// Monotonic (per replica) logical timestamp.
    pub ts: u64,
    /// `Some(v)` sets the value, `None` deletes the key.
    pub value: Option<String>,
}

/// Convergent state: `key -> (last timestamp, value)`.
pub type Registers = BTreeMap<String, (u64, Option<String>)>;

/// Applies one register mutation to the state. Last-write-wins by timestamp;
/// ties resolve to the op with the lexicographically greatest value so the map
/// remains deterministic.
pub fn apply_register(mut state: Registers, op: RegisterOp) -> Registers {
    let current = state.get(&op.key);
    match current {
        // Strictly older timestamp: ignore.
        Some((ts, _)) if op.ts < *ts => {}
        // Equal timestamp: keep the lexicographically greatest value, so the
        // final state is a pure function of the op set.
        Some((ts, value)) if op.ts == *ts && *value >= op.value => {}
        _ => {
            state.insert(op.key, (op.ts, op.value));
        }
    }
    state
}

/// Applies a batch of operations sequentially and returns the converged state.
pub fn apply_register_batch(
    mut state: Registers,
    ops: impl IntoIterator<Item = RegisterOp>,
) -> Registers {
    for op in ops {
        state = apply_register(state, op);
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(key: &str, ts: u64, value: Option<&str>) -> RegisterOp {
        RegisterOp {
            key: key.into(),
            ts,
            value: value.map(str::to_owned),
        }
    }

    #[test]
    fn last_writer_wins_by_timestamp() {
        let mut state = Registers::default();
        state = apply_register(state, op("k", 5, Some("old")));
        state = apply_register(state, op("k", 9, Some("new")));
        assert_eq!(state.get("k"), Some(&(9, Some("new".into()))));
    }

    #[test]
    fn earlier_timestamp_is_ignored() {
        let mut state = Registers::default();
        state = apply_register(state, op("k", 9, Some("new")));
        state = apply_register(state, op("k", 5, Some("old")));
        assert_eq!(state.get("k"), Some(&(9, Some("new".into()))));
    }

    #[test]
    fn delete_wins_only_once_reapplied_writers_revive() {
        let mut state = Registers::default();
        state = apply_register(state, op("k", 10, Some("v")));
        state = apply_register(state, op("k", 12, None));
        assert_eq!(state.get("k"), Some(&(12, None)));
        state = apply_register(state, op("k", 15, Some("v2")));
        assert_eq!(state.get("k"), Some(&(15, Some("v2".into()))));
    }

    #[test]
    fn register_is_commutative_and_idempotent() {
        let ops = vec![
            op("a", 1, Some("x")),
            op("a", 3, None),
            op("a", 2, Some("y")),
            op("b", 7, Some("z")),
        ];
        let a = apply_register_batch(Registers::default(), ops.iter().cloned());
        assert_crdt_commutative(
            Registers::default(),
            ops[0].clone(),
            ops[1].clone(),
            apply_register,
        );
        for op in &ops {
            assert_crdt_idempotent(Registers::default(), op.clone(), apply_register);
        }
        let _ = a;
    }
}

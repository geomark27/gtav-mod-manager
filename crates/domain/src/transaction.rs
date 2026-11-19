//! Transaction and step kinds, statuses and state machines.

use crate::{Error, Result};

persisted_enum!(
    /// What a transaction does.
    TransactionKind, "transaction kind" {
        Adopt => "adopt",
        Enable => "enable",
        Disable => "disable",
        Group => "group",
        Profile => "profile",
        Restore => "restore",
    }
);

persisted_enum!(
    /// Lifecycle status of a transaction.
    TransactionStatus, "transaction status" {
        Open => "open",
        Committing => "committing",
        Committed => "committed",
        RollingBack => "rolling_back",
        RolledBack => "rolled_back",
        NeedsAttention => "needs_attention",
    }
);

persisted_enum!(
    /// Kind of a transaction step.
    StepKind, "step kind" {
        CaptureToVault => "capture_to_vault",
        EnsureDir => "ensure_dir",
        PlaceFromVault => "place_from_vault",
        TombstoneGameFile => "tombstone_game_file",
    }
);

persisted_enum!(
    /// Lifecycle status of a transaction step.
    StepStatus, "step status" {
        Pending => "pending",
        Started => "started",
        Done => "done",
        Undone => "undone",
    }
);

impl TransactionStatus {
    /// True when `self -> next` is an allowed transition.
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Open, Self::Committing)
                | (Self::Open, Self::RollingBack)
                | (Self::Committing, Self::Committed)
                | (Self::Committing, Self::NeedsAttention)
                | (Self::RollingBack, Self::RolledBack)
                | (Self::RollingBack, Self::NeedsAttention)
        )
    }

    /// Returns `next` when the transition is allowed, `InvalidTransition` otherwise.
    pub fn transition(self, next: Self) -> Result<Self> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(Error::InvalidTransition {
                entity: "transaction",
                from: self.as_str(),
                to: next.as_str(),
            })
        }
    }

    /// True for final statuses (`Committed`, `RolledBack`).
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Committed | Self::RolledBack)
    }

    /// True when startup recovery must act on a transaction in this status.
    pub const fn needs_recovery(self) -> bool {
        matches!(self, Self::Open | Self::Committing | Self::RollingBack)
    }
}

impl StepStatus {
    /// True when `self -> next` is an allowed transition.
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Started)
                | (Self::Started, Self::Done)
                | (Self::Started, Self::Undone)
                | (Self::Done, Self::Undone)
        )
    }

    /// Returns `next` when the transition is allowed, `InvalidTransition` otherwise.
    pub fn transition(self, next: Self) -> Result<Self> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(Error::InvalidTransition {
                entity: "transaction step",
                from: self.as_str(),
                to: next.as_str(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::assert_enum_contract;
    use TransactionStatus as T;

    #[test]
    fn enum_contracts() {
        assert_enum_contract(
            &TransactionKind::ALL,
            &["adopt", "enable", "disable", "group", "profile", "restore"],
            TransactionKind::as_str,
        );
        assert_enum_contract(
            &TransactionStatus::ALL,
            &[
                "open",
                "committing",
                "committed",
                "rolling_back",
                "rolled_back",
                "needs_attention",
            ],
            TransactionStatus::as_str,
        );
        assert_enum_contract(
            &StepKind::ALL,
            &[
                "capture_to_vault",
                "ensure_dir",
                "place_from_vault",
                "tombstone_game_file",
            ],
            StepKind::as_str,
        );
        assert_enum_contract(
            &StepStatus::ALL,
            &["pending", "started", "done", "undone"],
            StepStatus::as_str,
        );
    }

    #[test]
    fn transaction_transition_table() {
        let allowed = [
            (T::Open, T::Committing),
            (T::Open, T::RollingBack),
            (T::Committing, T::Committed),
            (T::Committing, T::NeedsAttention),
            (T::RollingBack, T::RolledBack),
            (T::RollingBack, T::NeedsAttention),
        ];
        for from in T::ALL {
            for to in T::ALL {
                let expected = allowed.contains(&(from, to));
                assert_eq!(from.can_transition_to(to), expected, "{from} -> {to}");
                match from.transition(to) {
                    Ok(v) => assert!(expected && v == to),
                    Err(Error::InvalidTransition {
                        entity,
                        from: f,
                        to: t,
                    }) => {
                        assert!(!expected);
                        assert_eq!((entity, f, t), ("transaction", from.as_str(), to.as_str()));
                    }
                    Err(e) => unreachable!("unexpected error {e}"),
                }
            }
        }
    }

    #[test]
    fn terminal_and_recovery() {
        for s in T::ALL {
            assert_eq!(
                s.is_terminal(),
                matches!(s, T::Committed | T::RolledBack),
                "{s}"
            );
            assert_eq!(
                s.needs_recovery(),
                matches!(s, T::Open | T::Committing | T::RollingBack),
                "{s}"
            );
        }
    }

    #[test]
    fn step_transition_table() {
        use StepStatus as S;
        let allowed = [
            (S::Pending, S::Started),
            (S::Started, S::Done),
            (S::Started, S::Undone),
            (S::Done, S::Undone),
        ];
        for from in S::ALL {
            for to in S::ALL {
                let expected = allowed.contains(&(from, to));
                assert_eq!(from.can_transition_to(to), expected, "{from} -> {to}");
                assert_eq!(from.transition(to).is_ok(), expected, "{from} -> {to}");
            }
        }
    }
}

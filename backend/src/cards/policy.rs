//! Who may see and change a card. Every card handler goes through here.
//!
//! - A card is visible to its owner and, once shared, to its household.
//! - Anyone who can see a card can edit it (favourites are per user anyway).
//! - Only the owner can delete it, or share/unshare it with their household.
//!
//! Deleted cards are invisible to everyone.

use uuid::Uuid;

/// The viewer: a user and the household they belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewer {
    pub user_id: Uuid,
    pub household_id: Option<Uuid>,
}

/// The parts of a card that decide access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ownership {
    pub owner_id: Uuid,
    pub household_id: Option<Uuid>,
    pub deleted: bool,
}

pub fn can_view(viewer: Viewer, card: Ownership) -> bool {
    if card.deleted {
        return false;
    }
    card.owner_id == viewer.user_id
        || (card.household_id.is_some() && card.household_id == viewer.household_id)
}

pub fn can_edit(viewer: Viewer, card: Ownership) -> bool {
    can_view(viewer, card)
}

pub fn can_delete(viewer: Viewer, card: Ownership) -> bool {
    !card.deleted && card.owner_id == viewer.user_id
}

pub fn can_share(viewer: Viewer, card: Ownership) -> bool {
    can_delete(viewer, card)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: Uuid = Uuid::from_u128(1);
    const BEA: Uuid = Uuid::from_u128(2);
    const CARLO: Uuid = Uuid::from_u128(3);
    const HOME: Uuid = Uuid::from_u128(10);
    const OTHER_HOME: Uuid = Uuid::from_u128(11);

    fn viewer(user_id: Uuid, household_id: Uuid) -> Viewer {
        Viewer {
            user_id,
            household_id: Some(household_id),
        }
    }

    fn card(household_id: Option<Uuid>, deleted: bool) -> Ownership {
        Ownership {
            owner_id: ALICE,
            household_id,
            deleted,
        }
    }

    /// (viewer, card) → (view, edit, delete, share)
    #[test]
    fn access_matrix() {
        let cases = [
            (
                "owner, private",
                viewer(ALICE, HOME),
                card(None, false),
                (true, true, true, true),
            ),
            (
                "owner, shared",
                viewer(ALICE, HOME),
                card(Some(HOME), false),
                (true, true, true, true),
            ),
            (
                "member, private",
                viewer(BEA, HOME),
                card(None, false),
                (false, false, false, false),
            ),
            (
                "member, shared",
                viewer(BEA, HOME),
                card(Some(HOME), false),
                (true, true, false, false),
            ),
            (
                "stranger, shared",
                viewer(CARLO, OTHER_HOME),
                card(Some(HOME), false),
                (false, false, false, false),
            ),
            (
                "owner, deleted",
                viewer(ALICE, HOME),
                card(None, true),
                (false, false, false, false),
            ),
            (
                "member, deleted shared",
                viewer(BEA, HOME),
                card(Some(HOME), true),
                (false, false, false, false),
            ),
        ];

        for (name, viewer, card, expected) in cases {
            let actual = (
                can_view(viewer, card),
                can_edit(viewer, card),
                can_delete(viewer, card),
                can_share(viewer, card),
            );
            assert_eq!(actual, expected, "{name}");
        }
    }

    #[test]
    fn a_viewer_without_household_never_matches_an_unshared_card() {
        let lone = Viewer {
            user_id: BEA,
            household_id: None,
        };
        assert!(!can_view(lone, card(None, false)));
    }
}

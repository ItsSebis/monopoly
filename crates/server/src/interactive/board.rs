//! `GET /board`: static mechanical board data, serialized once from
//! `Board::standard()`. There's no space-name table anywhere in
//! `monopoly-engine` (`crates/engine/src/board.rs` only tracks the mechanical
//! `SpaceKind` data rent/purchase logic actually needs) — `name` is left out
//! rather than invented here, matching this endpoint's brief of exposing
//! whatever the engine already has, not growing new engine-side data for a
//! future GUI to render labels with.

use monopoly_engine::board::{Board, ColorGroup, SpaceKind, BOARD_SIZE};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BoardSpaceDto {
    pub index: usize,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<ColorGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_rent: Option<u32>,
    /// Rent with 1-4 houses (indices 0-3) and with a hotel (index 4) — see
    /// `SpaceKind::Street`'s own doc comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub house_rent: Option<[u32; 5]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub house_cost: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mortgage_value: Option<u32>,
}

/// `label` plus the street-only fields (`group`/`base_rent`/`house_rent`/
/// `house_cost`, all `None` for anything but `SpaceKind::Street`).
type LabelAndStreetFields = (
    &'static str,
    Option<ColorGroup>,
    Option<u32>,
    Option<[u32; 5]>,
    Option<u32>,
);

fn dto(index: usize, kind: SpaceKind) -> BoardSpaceDto {
    // `price`/`mortgage_value` apply the same way to every ownable kind
    // (`SpaceKind::price` already covers Street/Railroad/Utility uniformly),
    // so they're computed once here rather than per-variant below.
    let price = kind.price();
    let mortgage_value = price.map(|p| p / 2);
    let (label, group, base_rent, house_rent, house_cost): LabelAndStreetFields = match kind {
        SpaceKind::Go => ("go", None, None, None, None),
        SpaceKind::Street {
            group,
            base_rent,
            house_rent,
            house_cost,
            ..
        } => (
            "street",
            Some(group),
            Some(base_rent),
            Some(house_rent),
            Some(house_cost),
        ),
        SpaceKind::Railroad { .. } => ("railroad", None, None, None, None),
        SpaceKind::Utility { .. } => ("utility", None, None, None, None),
        SpaceKind::IncomeTax => ("income_tax", None, None, None, None),
        SpaceKind::LuxuryTax => ("luxury_tax", None, None, None, None),
        SpaceKind::Chance => ("chance", None, None, None, None),
        SpaceKind::CommunityChest => ("community_chest", None, None, None, None),
        SpaceKind::Jail => ("jail", None, None, None, None),
        SpaceKind::FreeParking => ("free_parking", None, None, None, None),
        SpaceKind::GoToJail => ("go_to_jail", None, None, None, None),
    };
    BoardSpaceDto {
        index,
        kind: label,
        group,
        price,
        base_rent,
        house_rent,
        house_cost,
        mortgage_value,
    }
}

pub fn board_dto() -> Vec<BoardSpaceDto> {
    let board = Board::standard();
    (0..BOARD_SIZE).map(|i| dto(i, board.space(i))).collect()
}

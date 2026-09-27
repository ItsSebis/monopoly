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

fn dto(index: usize, kind: SpaceKind) -> BoardSpaceDto {
    let blank = BoardSpaceDto {
        index,
        kind: "",
        group: None,
        price: None,
        base_rent: None,
        house_rent: None,
        house_cost: None,
        mortgage_value: None,
    };
    match kind {
        SpaceKind::Go => BoardSpaceDto {
            kind: "go",
            ..blank
        },
        SpaceKind::Street {
            group,
            price,
            base_rent,
            house_rent,
            house_cost,
        } => BoardSpaceDto {
            kind: "street",
            group: Some(group),
            price: Some(price),
            base_rent: Some(base_rent),
            house_rent: Some(house_rent),
            house_cost: Some(house_cost),
            mortgage_value: Some(price / 2),
            ..blank
        },
        SpaceKind::Railroad { price } => BoardSpaceDto {
            kind: "railroad",
            price: Some(price),
            mortgage_value: Some(price / 2),
            ..blank
        },
        SpaceKind::Utility { price } => BoardSpaceDto {
            kind: "utility",
            price: Some(price),
            mortgage_value: Some(price / 2),
            ..blank
        },
        SpaceKind::IncomeTax => BoardSpaceDto {
            kind: "income_tax",
            ..blank
        },
        SpaceKind::LuxuryTax => BoardSpaceDto {
            kind: "luxury_tax",
            ..blank
        },
        SpaceKind::Chance => BoardSpaceDto {
            kind: "chance",
            ..blank
        },
        SpaceKind::CommunityChest => BoardSpaceDto {
            kind: "community_chest",
            ..blank
        },
        SpaceKind::Jail => BoardSpaceDto {
            kind: "jail",
            ..blank
        },
        SpaceKind::FreeParking => BoardSpaceDto {
            kind: "free_parking",
            ..blank
        },
        SpaceKind::GoToJail => BoardSpaceDto {
            kind: "go_to_jail",
            ..blank
        },
    }
}

pub fn board_dto() -> Vec<BoardSpaceDto> {
    let board = Board::standard();
    (0..BOARD_SIZE).map(|i| dto(i, board.space(i))).collect()
}

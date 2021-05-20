/// Defines messages for sending and receiving to and from a user
use serde::{Serialize, Deserialize};

use crate::game::{Mark, Square};

/// Only message we receive is a move
#[derive(Serialize, Deserialize, Debug)]
pub enum ReceiveMessage {
    Move {pos: Square},
}

/// Different messages to send to players
#[derive(Serialize, Deserialize, Debug)]
pub enum SendMessage {
    Move {mark: Mark, pos: Square},
    StartGame {mark: Mark, first: bool},
    GameOver {winner: bool, draw: bool},
    PlayerLeft,
}
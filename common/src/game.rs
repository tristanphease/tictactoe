const BOARD_SIZE: usize = 3;
pub const NUM_PLAYERS: usize = 2;

use serde::{Deserialize, Serialize};

/// Represents a tictactoe game
pub struct Game<T> {
    players: Vec<Player<T>>,
    board: Board,
    curr_player: usize,
    ended: bool,
}

impl <T: PartialEq + Copy> Game<T> {
    /// Create a new game from a vector of players and the first player
    /// Players have a generic type that indicates what type is their identification
    pub fn new(players: Vec<Player<T>>, first: usize) -> Self {
        Self {
            players: players,
            board: Board::new(),
            curr_player: first,
            ended: false,
        }
    }

    /// Gets a reference to the player that's currently playing
    pub fn get_curr_player(&self) -> &Player<T> {
        &self.players[self.curr_player]
    }

    /// Gets whether a move from a player is a valid move
    pub fn can_move(&self, square: &Square, player_id: T) -> bool {
        if !self.ended && self.get_curr_player().id() == player_id {
            let pos = self.board.get_pos_coords(square.x, square.y);
            if pos.is_some() {
                if pos.unwrap() == Mark::Empty {
                    return true;
                }
            }
        }
        
        false
    }

    /// Makes the move using the current player
    /// Assumes can_move has been called
    pub fn make_move(&mut self, square: &Square) -> Option<GameResult> {
        let player = &self.players[self.curr_player];
        self.board.set_pos(square.x, square.y, player.mark);
        self.curr_player = (self.curr_player + 1) % NUM_PLAYERS;

        //check if someone won
        let result = self.board.game_over();
        if result.is_some() {self.ended = true}

        result
    }

    /// Gets a vector of player ids in the game
    pub fn get_player_ids(&self) -> Vec<T> {
        let mut ids = Vec::new();
        for player in &self.players {
            ids.push(player.id);
        }
        ids
    }

    /// Gets a mark a player uses
    pub fn get_player_mark(&self, id: T) -> Option<Mark> {
        for player in self.players.iter() {
            if player.id == id {
                return Some(player.mark);
            }
        }
        None
    }

    /// If a player leaves, just end the game early
    pub fn player_left(&mut self) {
        self.ended = true;
    }
}

/// Enum for a result of a game
#[derive(Debug, Clone, Copy)]
pub enum GameResult {
    CrossWon,
    NoughtWon,
    Draw,
}

impl GameResult {
    /// Get a game result from the mark winner
    fn from_mark(mark: Mark) -> GameResult {
        match mark {
            Mark::Cross => GameResult::CrossWon,
            Mark::Nought => GameResult::NoughtWon,
            Mark::Empty => panic!("Invalid"),
        }
    }
}

/// Defines a player with a mark and an id
/// The id is a generic type
#[derive(Debug, Clone)]
pub struct Player<T> {
    mark: Mark,
    id: T,
}

impl<T: PartialEq + Copy> Player<T> {
    /// Create new player
    pub fn new(mark: Mark, id: T) -> Self {
        Self {
            mark: mark,
            id: id,
        }
    }

    /// Get id of player
    pub fn id(&self) -> T {
        self.id
    }
}

/// Mark on a noughts and crosses boar
#[derive(PartialEq, Clone, Copy, Debug)]
#[derive(Serialize, Deserialize)]
pub enum Mark {
    Nought,
    Cross,
    Empty,
}

/// Represents a board
#[derive(Debug)]
pub struct Board {
    width: usize,
    height: usize,
    array: [Mark; BOARD_SIZE*BOARD_SIZE],
}

impl Board {
    /// Create a new board
    pub fn new() -> Self {
        Self {
            width: BOARD_SIZE,
            height: BOARD_SIZE,
            array: [Mark::Empty; BOARD_SIZE*BOARD_SIZE],
        }
    }

    /// Gets the contents on the board given an x and y coord of the position
    pub fn get_pos_coords(&self, x: usize, y: usize) -> Option<Mark> {
        if x >= BOARD_SIZE || y >= BOARD_SIZE {
            return None;
        }
        Some(self.array[y * BOARD_SIZE + x]) 
    }
    
    /// Gets the contents on the board given a single coord of the position
    pub fn get_pos(&self, spot: usize) -> Option<Mark> {
        self.get_pos_coords(spot % self.width, spot / self.width)
    }
    
    /// Sets the position on a board
    pub fn set_pos(&mut self, x: usize, y: usize, new_pos: Mark) {
        self.array[y * BOARD_SIZE + x] = new_pos;
    }

    /// Checks whether the game is over, returning what the result is
    /// Could be more cleanly implemented
    pub fn game_over(&self) -> Option<GameResult> {
        //check lines and diagonals
        //first check horizontal
        for y in 0..BOARD_SIZE {
            let first_mark = self.array[y*3];
            if first_mark != Mark::Empty {
                let mut count = 1;
                for x in 1..BOARD_SIZE {
                    if self.array[y*3 + x] == first_mark {
                        count += 1;
                    }
                }
                if count == BOARD_SIZE {
                    return Some(GameResult::from_mark(first_mark));
                }
            }
        }
        //check vertical
        for x in 0..BOARD_SIZE {
            let first_mark = self.array[x];
            if first_mark != Mark::Empty {
                let mut count = 1;
                for y in 1..BOARD_SIZE {
                    if self.array[y*3 + x] == first_mark {
                        count += 1;
                    }
                }
                if count == BOARD_SIZE {
                    return Some(GameResult::from_mark(first_mark));
                }
            }
        }

        //check diagonals
        //top left to bottom right diagonal
        let first_mark = self.array[0];
        if first_mark != Mark::Empty {
            let mut count = 1;
            for x in 1..BOARD_SIZE {
                let y = x;
                if self.array[y*3 + x] == first_mark {
                    count += 1;
                }
            }
            if count == BOARD_SIZE {
                return Some(GameResult::from_mark(first_mark));
            }
        }

        //from top right to bottom left
        let first_mark = self.array[BOARD_SIZE-1];
        if first_mark != Mark::Empty {
            let mut count = 1;
            for x in (0..BOARD_SIZE-1).rev() {
                let y = BOARD_SIZE - 1 - x;
                if self.array[y*3 + x] == first_mark {
                    count += 1;
                }
            }
            if count == BOARD_SIZE {
                return Some(GameResult::from_mark(first_mark));
            }
        }

        for value in self.array.iter() {
            if *value == Mark::Empty {
                return None;
            }
        }

        Some(GameResult::Draw)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Square {
    x: usize,
    y: usize,
}

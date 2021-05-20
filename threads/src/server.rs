use std::{collections::HashSet, io::ErrorKind, net::{TcpListener, TcpStream}};
use std::thread::spawn;
use std::sync::Mutex;
use std::sync::Arc;

use std::collections::HashMap;

use tungstenite::{HandshakeError, server::accept};
use tungstenite::error::Error;
use tungstenite::Message;
use tungstenite::protocol::WebSocket;

use common::{game::{Game, GameResult, Mark, NUM_PLAYERS, Player}, message::{ReceiveMessage, SendMessage}};

/// A server
pub struct Server {
    websockets: HashMap<usize, WebSocket<TcpStream>>,
    counter: usize,
    messages: HashMap<usize, Vec<String>>,
    lobby: HashSet<usize>,
    games: Vec<Game<usize>>,
    //map from id to the game for efficiency
    game_map: HashMap<usize, usize>,
}

impl Server {
    /// Creates a new server
    pub fn new() -> Self {
        Self {
            websockets: HashMap::new(),
            counter: 0,
            messages: HashMap::new(),
            lobby: HashSet::new(),
            games: Vec::new(),
            game_map: HashMap::new(),
        }
    }

    /// Disconnects a user from the server
    pub fn disconnect(&mut self, id: usize) {
        let removed = self.lobby.remove(&id);
        if !removed {
            let game_index = *self.game_map.get(&id).unwrap();

            //send to all other players in game that the player disconnected
            println!("User {} left game", id);

            let message_str = serde_json::to_string(&SendMessage::PlayerLeft).unwrap();
            for player_id in self.games[game_index].get_player_ids().iter() {
                if player_id != &id {
                    if let Some(vec) = self.messages.get_mut(&player_id) {
                        vec.push(message_str.clone());
                    }
                }
            }
            self.games[game_index].player_left();
        } else {
            println!("User {} removed from lobby", id);
        }

        self.websockets.remove(&id);
        self.messages.remove(&id);
    }
}

/// Starts a server
pub fn start_server(server: Server) {
    let listener = TcpListener::bind("127.0.0.1:8000").unwrap();

    let server_arc = Arc::new(Mutex::new(server));
    
    'listen: for stream in listener.incoming() {

        let stream = stream.unwrap();
        let block_res = stream.set_nonblocking(true);
        if block_res.is_err() {panic!("Couldn't set to blocking");}

        let mut websocket_res = accept(stream);
        while websocket_res.is_err() {
            let error = websocket_res.unwrap_err();
            match error {
                HandshakeError::Interrupted(mid) => {
                    println!("Interrupted so restarting");
                    websocket_res = mid.handshake();
                },
                HandshakeError::Failure(_) => {
                    //failed, so just continue
                    continue 'listen;
                },
            }
        }
        let websocket = websocket_res.unwrap();

        let mut server = server_arc.lock().unwrap();

        //set id and increment counter
        let id = server.counter;
        server.counter += 1;

        //add to hashmap
        server.websockets.insert(id, websocket);
        server.messages.insert(id, Vec::new());
        server.lobby.insert(id);

        if server.lobby.len() == NUM_PLAYERS {
            start_game(&mut server);
        } 
        
        println!("user connected to server, id = {}", id);

        setup_client(server_arc.clone(), id);
        
    }
}

/// Sets up a new thread for a client
fn setup_client(server_arc: Arc<Mutex<Server>>, id: usize) {
    //thread for waiting for new messages from the server to send to the client
    spawn(move|| { 'whole: loop {
        
        let mut server = server_arc.lock().unwrap();

        let mut messages = server.messages.get_mut(&id)
            .unwrap().iter()
            .map(|str| str.to_string()).collect::<Vec<_>>();
        
        server.messages.get_mut(&id).unwrap().clear();

        let websocket = server.websockets.get_mut(&id).unwrap();
        
        while messages.len() > 0 {
            let _res = websocket.write_message(Message::Text(messages.remove(0)));
        }

        match websocket.read_message() {
            Ok(msg) => {
                handle_message(&mut server, id, msg);
            }
            Err(error) => {
                match error {
                    Error::ConnectionClosed => {
                        server.disconnect(id);
                        println!("User {} disconnected", id);
                        //remove from lobby/game
                        break 'whole;
                    },
                    Error::Io(io_err) => {
                        match io_err.kind() {
                            ErrorKind::WouldBlock => {},
                            _ => {
                                println!("Misc io error: {:?}", io_err);
                            }
                        }
                    },
                    Error::AlreadyClosed => {
                        server.disconnect(id);
                        println!("disconnecting since already closed");
                        //remove from lobby/game
                        break 'whole;
                    },
                    _ => {
                        println!("Other error - {:?}", error);
                    },
                }
            }
        }
    } });
}

/// Handles a message from a user
/// Far too nested, doesn't handle errors effectively
fn handle_message(server: &mut Server, id: usize, msg: Message) {
    match msg {
        Message::Text(message) => {
            match serde_json::from_str::<ReceiveMessage>(&message) {
                Ok(message) => {
                    match message {
                        ReceiveMessage::Move { pos } => {
                            let index_op = server.game_map.get(&id);
                            match index_op {
                                Some(index) => {
                                    let index = *index;
                                    let game = &mut server.games[index];
                                    if game.can_move(&pos, id) {
                                        let game_result = game.make_move(&pos);

                                        let mark = game.get_player_mark(id).unwrap();
                                        let message = SendMessage::Move{mark: mark, pos: pos};

                                        let msg_str = serde_json::to_string(&message).unwrap();

                                        //dispatch to all except one that made the move
                                        send_all_but_one(server, id, &msg_str, Some(&index));

                                        if game_result.is_some() {
                                            match game_result.unwrap() {
                                                GameResult::CrossWon => {
                                                    for player_id in server.games[index].get_player_ids() {
                                                        let won = Mark::Cross == server.games[index].get_player_mark(player_id).unwrap();
                                                        let message = SendMessage::GameOver {winner: won, draw: false};
                                                        let msg_str = serde_json::to_string(&message).unwrap();
                                                        send_one(server, player_id, &msg_str);
                                                    }
                                                },
                                                GameResult::NoughtWon => {
                                                    for player_id in server.games[index].get_player_ids() {
                                                        let won = Mark::Nought == server.games[index].get_player_mark(player_id).unwrap();
                                                        let message = SendMessage::GameOver {winner: won, draw: false};
                                                        let msg_str = serde_json::to_string(&message).unwrap();
                                                        send_one(server, player_id, &msg_str);
                                                    }
                                                },
                                                GameResult::Draw => {
                                                    let message = SendMessage::GameOver{winner: false, draw: true};
                                                    let msg_str = serde_json::to_string(&message).unwrap();
                                                    send_all(server, &msg_str, Some(&index));
                                                },
                                            };

                                            //end game here

                                        }
                                    }
                                },
                                None => {
                                    //probably means the user isn't in a game
                                }
                            }
                        },
                    }
                },
                Err(_) => {
                    println!("couldn't parse {:?}", message);
                },
            }
        },
        _ => {},
    }
}

/// Send a message to all users in a server or in a game in a server
/// Option for the game index, if None send to all in server
pub fn send_all(server: &mut Server, message: &String, game_index: Option<&usize>) {
    match game_index {
        Some(game_index) => {
            for id in server.games[*game_index].get_player_ids().iter() {
                let vec = server.messages.get_mut(id).unwrap();
                vec.push(message.clone());
            }
        },
        None => {
            for vec in server.messages.values_mut() {
                vec.push(message.clone());
            }
        }
    }
}

/// Send a message to all users in a server/game except one
/// Option for the game index, if None send to all in server(but one)
pub fn send_all_but_one(server: &mut Server, id: usize, message: &String, game_index: Option<&usize>) {
    match game_index {
        Some(game_index) => {
            for player_id in server.games[*game_index].get_player_ids().iter() {
                if player_id != &id {
                    let vec = server.messages.get_mut(player_id).unwrap();
                    vec.push(message.clone());
                }    
            }
        },
        None => {
            for (player_id, vec) in server.messages.iter_mut() {
                if player_id != &id {
                    vec.push(message.clone());
                }
            }
        }
    }
}

/// Send a message to a single user in a server
pub fn send_one(server: &mut Server, id: usize, message: &String) {
    //println!("Sending {} to {}", message, id);
    let vec = server.messages.get_mut(&id);
    
    match vec {
        Some(vec) => {
            vec.push(message.clone());
        },
        None => {
            println!("Invalid id - {} for message {}", id, message);
        },
    }
}

/// Starts a game
fn start_game(server: &mut Server) {
    let mut persons = Vec::new();

    let first = fastrand::usize(0..NUM_PLAYERS);
    for (i, id) in server.lobby.iter().enumerate() {
        let messages = server.messages.get_mut(&id).unwrap();

        let (mark, first) = match i % 2 == 0 {
            true => (Mark::Cross, i == first),
            false => (Mark::Nought, i == first),
        };

        let player = Player::new(mark, *id);
        persons.push(player);

        let start_mess = SendMessage::StartGame {mark, first};

        messages.push(serde_json::to_string(&start_mess).unwrap());
    }
    

    let game = Game::new(persons, first);

    for id in server.lobby.iter() {
        server.game_map.insert(*id, server.games.len());
    }
    server.lobby.clear();

    server.games.push(game);
}

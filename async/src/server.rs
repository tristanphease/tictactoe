use std::{collections::{HashMap, HashSet}, net::SocketAddr, sync::{Arc, Mutex}};

use async_std::{net::{TcpListener, TcpStream}};
use async_std::task;
use common::{game::{Game, GameResult, Mark, NUM_PLAYERS, Player}, message::{ReceiveMessage, SendMessage}};
use futures::{StreamExt, TryStreamExt, channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded}, future};
use async_tungstenite::tungstenite::protocol::Message;

/// A server
pub struct Server {
    messages: HashMap<SocketAddr, UnboundedSender<Message>>,
    lobby: HashSet<SocketAddr>,
    games: Vec<Game<SocketAddr>>,
    game_map: HashMap<SocketAddr, usize>, //id to index of games
}

impl Server {
    /// Creates a new server
    pub fn new() -> Self {
        Server {
            messages: HashMap::new(),
            lobby: HashSet::new(),
            games: Vec::new(),
            game_map: HashMap::new(),
        }
    }

    /// Sends a message to a single user
    pub fn send_one(&self, addr: SocketAddr, msg: Message) {
        match self.messages.get(&addr) {
            Some(sender) => {
                sender.unbounded_send(msg.clone()).unwrap();
            },
            None => println!("Invalid address - {}", addr),
        }
    }

    /// Sends a message to all users in a server/game
    pub fn send_all(&self, msg: Message, game_index: Option<usize>) {
        match game_index {
            Some(index) => {
                let game = &self.games[index];
                for addr in game.get_player_ids() {
                    self.send_one(addr, msg.clone());
                }
            },
            None => {
                for sender in self.messages.values() {
                    sender.unbounded_send(msg.clone()).unwrap();
                }
            },
        }
    }

    /// Sends a message to all user except one in a server/game
    pub fn send_all_but_one(&self, msg: Message, addr_not_send: SocketAddr, game_index: Option<usize>) {
        match game_index {
            Some(index) => {
                let game = &self.games[index];
                for addr in game.get_player_ids() {
                    if addr != addr_not_send {
                        self.send_one(addr, msg.clone());
                    }
                }
            },
            None => {
                for sender in self.messages.values() {
                    sender.unbounded_send(msg.clone()).unwrap();
                }
            },
        }
    }

    /// Disconnects a user from the lobby/a game
    pub fn disconnect(&mut self, addr: SocketAddr) {
        let removed = self.lobby.remove(&addr);
        if !removed {
            let game_index = *self.game_map.get(&addr).unwrap();

            //send to all other players in game that the player disconnected
            println!("User {} left game", addr);

            let message_str = serde_json::to_string(&SendMessage::PlayerLeft).unwrap();
            for player_addr in self.games[game_index].get_player_ids().iter() {
                if player_addr != &addr {
                    if let Some(sender) = self.messages.get(&player_addr) {
                        sender.unbounded_send(Message::Text(message_str.clone())).unwrap();
                    }
                }
            }
            self.games[game_index].player_left();
        } else {
            println!("User {} removed from lobby", addr);
        }

        self.messages.remove(&addr);
    }
}

/// Starts a server
pub async fn start_server() {
    let try_socket = TcpListener::bind("127.0.0.1:8000").await;
    let listener = try_socket.expect("Failed to bind");

    let server = Server::new();
    let server_arc = Arc::new(Mutex::new(server));

    while let Ok((stream, addr)) = listener.accept().await {
        let server_clone = server_arc.clone();

        //do server new client stuff
        let mut server = server_clone.lock().unwrap();
        server.lobby.insert(addr);
        drop(server);

        task::spawn(handle_connection(stream, addr, server_clone));
    }
}

/// Handles an async connection
async fn handle_connection(stream: TcpStream, addr: SocketAddr, server: Arc<Mutex<Server>>) {
    println!("User {} connected", addr);
    let ws_stream = async_tungstenite::accept_async(stream)
        .await
        .expect("Error");
    
    let (tx, rx): (UnboundedSender<Message>, UnboundedReceiver<Message>) = unbounded();
    
    //block to drop after
    {
        let mut server = server.lock().unwrap();
        server.messages.insert(addr, tx);
        server.lobby.insert(addr);

        if server.lobby.len() == NUM_PLAYERS {
            start_game(&mut server);
        }
    }

    let (outgoing, incoming) = ws_stream.split();
    
    let broadcast_incoming = incoming
        .try_filter(|msg| {
            future::ready(!msg.is_close())
        })
        .try_for_each(|msg| {

            let mut server = server.lock().unwrap();
            handle_message(&mut server, addr, msg);
            future::ok(())
        });
    
    let receive = rx.map(Ok).forward(outgoing);
    
    //pin_mut!(broadcast_incoming, receive);
    future::select(broadcast_incoming, receive).await;
    
    server.lock().unwrap().disconnect(addr);

}

/// Starts a game given enough players have joined the lobby
fn start_game(server: &mut Server) {
    let mut persons = Vec::new();

    let first = fastrand::usize(0..NUM_PLAYERS);
    for (i, addr) in server.lobby.iter().enumerate() {
        let messages = server.messages.get(&addr).unwrap();

        let (mark, first) = match i % 2 == 0 {
            true => (Mark::Cross, i == first),
            false => (Mark::Nought, i == first),
        };

        let player = Player::new(mark, *addr);
        persons.push(player);

        let start_mess = SendMessage::StartGame {mark, first};

        messages.unbounded_send(Message::Text(serde_json::to_string(&start_mess).unwrap())).unwrap();
    }
    

    let game = Game::new(persons, first);

    for addr in server.lobby.iter() {
        server.game_map.insert(*addr, server.games.len());
    }
    server.lobby.clear();

    server.games.push(game);
}

/// Handles a message from a user
fn handle_message(server: &mut Server, addr: SocketAddr, msg: Message) {
    match msg {
        Message::Text(message) => {
            match serde_json::from_str::<ReceiveMessage>(&message) {
                Ok(text) => {
                    handle_receive(server, addr, text)
                },
                Err(_) => {},
            }
        },
        _ => {},
    }
}

/// Handles a receive message from a user
/// Here to avoid excessive nesting but doesn't really do effective error handling
fn handle_receive(server: &mut Server, addr: SocketAddr, msg: ReceiveMessage) {
    match msg {
        ReceiveMessage::Move {pos} => {
            let game_index = server.game_map.get(&addr);

            match game_index {
                Some(index) => {
                    let game = &mut server.games[*index];

                    if game.can_move(&pos, addr) {
                        let game_result = game.make_move(&pos);

                        let mark = game.get_player_mark(addr).unwrap();
                        let message = SendMessage::Move{mark: mark, pos: pos};

                        let msg_str = serde_json::to_string(&message).unwrap();

                        server.send_all_but_one(Message::Text(msg_str), addr, Some(*index));

                        match game_result {
                            Some(result) => {
                                match result {
                                    GameResult::CrossWon => {
                                        for player_addr in server.games[*index].get_player_ids() {
                                            let won = Mark::Cross == server.games[*index].get_player_mark(player_addr).unwrap();
                                            let message = SendMessage::GameOver{winner: won, draw: false};
                                            let msg = Message::Text(serde_json::to_string(&message).unwrap());
                                            server.send_one(player_addr, msg);
                                        }
                                    },
                                    GameResult::NoughtWon => {
                                        for player_addr in server.games[*index].get_player_ids() {
                                            let won = Mark::Nought == server.games[*index].get_player_mark(player_addr).unwrap();
                                            let message = SendMessage::GameOver{winner: won, draw: false};
                                            let msg = Message::Text(serde_json::to_string(&message).unwrap());
                                            server.send_one(player_addr, msg);
                                        }
                                    },
                                    GameResult::Draw => {
                                        let message = SendMessage::GameOver{winner: false, draw: false};
                                        let msg = Message::Text(serde_json::to_string(&message).unwrap());
                                        server.send_all(msg, Some(*index));
                                    },
                                }
                            },
                            None => {},
                        }
                    }
                },
                None => {},
            }
        }
    }
}
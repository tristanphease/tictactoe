use crate::server::Server;

use server::start_server;

mod server;

fn main() {
    let server = Server::new();

    start_server(server);
}

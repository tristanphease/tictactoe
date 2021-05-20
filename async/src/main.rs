mod server;

use futures::executor::block_on;
use server::start_server;

fn main() {
    block_on(start_server());
}

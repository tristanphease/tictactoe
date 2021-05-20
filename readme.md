Simple tictactoe websocket rust server with both an async and threaded implementation.

Uses the tungstenite/async-tungstenite crates for websocket integration.

Includes a very basic html client.

Definitely could be cleaner/more comprehensive but was mostly for learning rust stuff.

To play, run "cargo run -p tictactoe-threads" or "cargo run -p tictactoe-async" as wanted(default is async)
 then open up the client.html file in two browser tabs
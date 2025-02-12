# Multiplayer Snake Tutorial

![Platformer Game Movement Gif](kiwi-movement.gif)

Check out the finish product here: 

## Description

This project walks you through how to make a real-time multiplayer game using the **Turbo OS** Channel System. To get started with Turbo OS visit: [TURBO OS](https://os.turbo.computer/dashboard)

## The Basics

The Channel System registers specific users in and out, and allows them to share data between their games. The snake game is a simple implementation of a lobby system, where all players who join the game enter into the same channel.

This code connects players into the shared channel.
```rs
    // Subscribe to the channel
    let multiplayer_snake_channel =
        Channel::subscribe(PROGRAM_NAME, "snake_controller", "snake-channel");

    // Connect to channel
    if let Channel::Disconnected(ref conn) = multiplayer_snake_channel {
        state.joined_game = false;
        conn.connect();
    };
```
The local client for each player has a two way communication with the channel. It informs the channel whenever the player presses a key, telling the channel to turn the snake. The channel informs the clients whenever the snakes move, then the client draws all the graphics based on the data shared by the channel.

### Receiving Messages from the Channel

Once we are connected to the channel, we want to query it for messages in our go loop.

```rs
// Receive messages from the channel
if let Channel::Connected(ref conn) = multiplayer_snake_channel {
    while let Ok(Some(data)) = conn.recv() {
        // Parse message
        if let Ok(message) = SnakeChannelMessage::try_from_slice(&data) {
            match message {
                SnakeChannelMessage::StateUpdate(next_state) => {
                    state.snake_session = next_state;
                }
                SnakeChannelMessage::PlayerJoined() => {
                    state.joined_game = true;
                }
                SnakeChannelMessage::PlayerDied() => {
                    state.joined_game = false;
                }
            }
        }
    }
}
```
Channel messages can be any data type. Here we are using an enum `SnakeChannelMessage`

```rs
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
enum SnakeChannelMessage {
    StateUpdate(SnakeSession),
    PlayerJoined(),
    PlayerDied(),
}
```
`StateUpdate` Has all the information about the board state, so that the client knows where to draw the apples and snakes. Each player is always receiving this message at the same time as each other, so that the board always stays in sync between players.

The other two messages are sent only to a specific UserID when that player joins or dies.

### Sending Messages from the Channel
Inside of the channel we can interact with functions as needed. Any game state changes that are shared between players should be done on the channel to make sure they are performed the same way for all players.

Let's look at an example when a player joins the game. First, the player sends a message to the channel to join the game if they press start or click.

```rs
// Send a message to join the game
if !state.joined_game && (gp.start.just_pressed() || mouse(0).left.just_pressed()) {
    if let Channel::Connected(ref conn) = multiplayer_snake_channel {
        let msg = PlayerMessage::JoinGame;
        let _ = conn.send(&msg.try_to_vec().unwrap());
    }
}
```

Then the channel interprets that message to make sure the player is registered correctly. Then it sends a message back to the player that the player has joined, and the player's client can update as needed (in this case it stops showing the "press space to start" message).

```rs
PlayerMessage::JoinGame => {
    // Check if you are already in the game
    // If not, then create a snake and add it to the map
    if !player_snake_ids.contains_key(&user_id) {
        init_snake(&mut state.snakes, snake_id, state.grid_size);
        player_snake_ids.insert(user_id.clone(), snake_id);

        let msg = SnakeChannelMessage::PlayerJoined();
        os::server::channel_send(&user_id, &msg.try_to_vec().unwrap());
        snake_id += 1;
    }
}
```

Any data in the channel, like the player_snake_ids BTreeMap, will persist until the channel closes. The channel closes when there are no players connected.

Lastly, the client has to parse the message sent back from the channel. 

```rs
// Receive messages from the channel
if let Channel::Connected(ref conn) = multiplayer_snake_channel {
    while let Ok(Some(data)) = conn.recv() {
        // Parse message
        if let Ok(message) = SnakeChannelMessage::try_from_slice(&data) {
            match message {
                SnakeChannelMessage::StateUpdate(next_state) => {
                    state.snake_session = next_state;
                }
                SnakeChannelMessage::PlayerJoined() => {
                    state.joined_game = true;
                }
                SnakeChannelMessage::PlayerDied() => {
                    state.joined_game = false;
                }
            }
        }
    }
}
```

We use `try_from_slice(&data)` to evaluate the message, and then perform whatever logic we need to in the match statement.

### Moving snakes with Timeout
When we parse our channel messages we use the function `os::server::channel_recv_with_timeout(64) `. This will send a timeout error every 64ms. We can count on this to be a consistent timer when the game is running in the web, and for all players to stay in sync.

When we get the timeout error, we move the snakes, and then send the updated state to all players

```rs
// Handle a timeout error
Err(server::ChannelError::Timeout) => {
    if state.apples.len() == 0 {
        create_new_apple(&mut state.snakes, &mut state.apples, state.grid_size);
    }
    move_snakes(&mut state.snakes, state.grid_size);
    check_for_overlaps(
        &mut state.snakes,
        &mut state.apples,
        &mut player_snake_ids,
        &mut driver,
    );
    let msg = SnakeChannelMessage::StateUpdate(state.clone());
    os::server::channel_broadcast(&msg.try_to_vec().unwrap());
    did_update.clear();
}
```
If you don't need a recurring timer in your channel (e.g. if it only updates when players send an input) you should use ```channel_recv()``` instead.

### Next Steps

You can use this as the foundation for a realtime multiplayer game. Make sure to update `const PROGRAM_NAME` if you create a new program to build off of this. There are lots of ways to adapt this game without even adjusting the netcode, just by adding the data you need to the `SnakeSession` struct. Here are a few ideas:

- Change the character design and movement pattern
- Add a leaderboard and track a score for each connected user_id
- Make the players crash into eachother. Last one standing is the winner
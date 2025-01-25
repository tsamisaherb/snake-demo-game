#![allow(unexpected_cfgs)]

use crate::os::*;
use client::channel::*;
use std::collections::{BTreeMap, BTreeSet};

turbo::cfg! {r#"
    name = "snake-demo-game"
    version = "1.0.0"
    author = "Turbo"
    description = "Your first turbo os program"
    [settings]
    resolution = [512, 512]
    [turbo-os]
    api-url = "https://os.turbo.computer"
"#}

turbo::init! {
    struct GameState {
        //This is all updated by the server
        snake_session: struct SnakeSession {
            grid_size: u16,
            snakes: Vec<Snake>,
            apples: Vec<(u16, u16)>,
        },
        //This is tracked locally for each player
        joined_game: bool,
    } = {
        Self {
            snake_session: SnakeSession {
                grid_size: 16,
                snakes: Vec::new(),
                apples: Vec::new(),
            },
            joined_game: false,
        }
    }
}

const PROGRAM_NAME: &'static str = "snake-demo";
const CANVAS_WIDTH: u16 = 512;
const CANVAS_HEIGHT: u16 = 512;

turbo::go!({
    let mut state = GameState::load();

    // Subscribe to the channel
    let multiplayer_snake_channel =
        Channel::subscribe(PROGRAM_NAME, "snake_controller", "snake-channel");

    // Connect to channel
    if let Channel::Disconnected(ref conn) = multiplayer_snake_channel {
        state.joined_game = false;
        conn.connect();
    };

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

    // Use gamepad to detect player input
    let gp = gamepad(0);

    // Send a message to join the game
    if !state.joined_game && (gp.start.just_pressed() || mouse(0).left.just_pressed()) {
        if let Channel::Connected(ref conn) = multiplayer_snake_channel {
            let msg = PlayerMessage::JoinGame;
            let _ = conn.send(&msg.try_to_vec().unwrap());
        }
    }

    // Reset message for debugging purposes
    if gp.a.just_pressed() && gp.start.just_pressed() {
        if let Channel::Connected(ref conn) = multiplayer_snake_channel {
            let msg = PlayerMessage::ResetGame;
            let _ = conn.send(&msg.try_to_vec().unwrap());
        }
    }

    // Move the Snake from player input
    if gp.up.just_pressed() {
        if let Channel::Connected(ref conn) = multiplayer_snake_channel {
            let msg = PlayerMessage::ChangeDirection { dir: Direction::Up };
            let _ = conn.send(&msg.try_to_vec().unwrap());
        }
    } else if gp.down.just_pressed() {
        if let Channel::Connected(ref conn) = multiplayer_snake_channel {
            let msg = PlayerMessage::ChangeDirection {
                dir: Direction::Down,
            };
            let _ = conn.send(&msg.try_to_vec().unwrap());
        }
    } else if gp.left.just_pressed() {
        if let Channel::Connected(ref conn) = multiplayer_snake_channel {
            let msg = PlayerMessage::ChangeDirection {
                dir: Direction::Left,
            };
            let _ = conn.send(&msg.try_to_vec().unwrap());
        }
    } else if gp.right.just_pressed() {
        if let Channel::Connected(ref conn) = multiplayer_snake_channel {
            let msg = PlayerMessage::ChangeDirection {
                dir: Direction::Right,
            };
            let _ = conn.send(&msg.try_to_vec().unwrap());
        }
    }

    // The client draws all the graphics based on the state recieved from the server
    draw_snakes(&state.snake_session.snakes, state.snake_session.grid_size);
    draw_apples(&state.snake_session.apples, state.snake_session.grid_size);
    draw_text(state.joined_game);
    state.save();
});

/// Create a Snake. the ID is how we track which snake belongs to which player
fn init_snake(snakes: &mut Vec<Snake>, snake_id: u8) {
    let snake_size = 5;
    let starting_positions = vec![(5, 5); snake_size]; // 5 units at position (5,5)

    let snake = Snake {
        positions: starting_positions,
        direction: Direction::Right,
        snake_id,
    };

    snakes.push(snake);
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
enum PlayerMessage {
    JoinGame,
    ChangeDirection { dir: Direction },
    ResetGame,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
struct Snake {
    positions: Vec<(u16, u16)>,
    direction: Direction,
    snake_id: u8,
}

fn move_snakes(snakes: &mut Vec<Snake>, grid_size: u16) {
    let width = CANVAS_WIDTH / grid_size;
    let height = CANVAS_HEIGHT / grid_size;
    for snake in snakes.iter_mut() {
        let (head_x, head_y) = snake.positions[0];
        let new_head = match snake.direction {
            Direction::Up => (head_x, (height + head_y - 1) % height),
            Direction::Down => (head_x, (head_y + 1) % height),
            Direction::Left => ((width + head_x - 1) % width, head_y),
            Direction::Right => ((head_x + 1) % width, head_y),
        };
        snake.positions.insert(0, new_head);
        snake.positions.pop();
    }
}

fn check_for_overlaps(
    snakes: &mut Vec<Snake>,
    apples: &mut Vec<(u16, u16)>,
    player_snake_ids: &mut BTreeMap<String, u8>,
    driver: &mut String,
) {
    let mut to_remove = Vec::new();

    // Check if the Snake Head overlaps wnith any apples or with any part of itself
    for snake in snakes.iter_mut() {
        let head = snake.positions[0];
        if let Some(index) = apples.iter().position(|&apple| apple == head) {
            apples.remove(index);
            snake
                .positions
                .push(snake.positions[snake.positions.len() - 1]);
        }
        if snake.positions[1..].contains(&head) {
            to_remove.push(snake.snake_id);
        }
    }

    // Remove any snakes that have an overlap with themselves
    for id in to_remove {
        if let Some(user_id) = player_snake_ids
            .iter()
            .find(|(_, &snake_id)| snake_id == id)
            .map(|(user, _)| user.clone())
        {
            remove_player(id, player_snake_ids);
            if *driver == user_id {
                *driver = "".to_string();
            }
            snakes.retain(|snake| snake.snake_id != id);
            // Send a message back to the player that died, so that they know to switch to the
            let msg = SnakeChannelMessage::PlayerDied();
            os::server::channel_send(&user_id, &msg.try_to_vec().unwrap());
        }
    }
}

fn create_new_apple(snakes: &mut Vec<Snake>, apples: &mut Vec<(u16, u16)>, grid_size: u16) {
    let width = CANVAS_WIDTH / grid_size;
    let height = CANVAS_HEIGHT / grid_size;

    loop {
        let pos = (
            (os::server::random_number::<u16>() % width),
            (os::server::random_number::<u16>() % height),
        );
        let overlaps = snakes.iter().any(|snake| snake.positions.contains(&pos));
        if !overlaps {
            apples.push(pos);
            break;
        }
    }
}

fn draw_apples(apples: &Vec<(u16, u16)>, grid_size: u16) {
    for (x, y) in apples {
        circ!(
            x = x * grid_size + 2,
            y = y * grid_size + 2,
            d = grid_size - 4,
            color = 0x00FF00FF
        );
    }
}

fn remove_player(snake_id: u8, player_snake_ids: &mut BTreeMap<String, u8>) {
    if let Some(key) = player_snake_ids
        .iter()
        .find(|(_, &id)| id == snake_id)
        .map(|(k, _)| k.clone())
    {
        player_snake_ids.remove(&key);
    }
}

fn draw_snakes(snakes: &Vec<Snake>, grid_size: u16) {
    for snake in snakes {
        let color: u32 = match snake.snake_id % 6 {
            0 => 0x9370DBff, // Medium Purple
            1 => 0xFF69B4ff, // Hot Pink
            2 => 0x20B2AAff, // Light Sea Green
            3 => 0xDDA0DDff, // Plum
            4 => 0xFF8C00ff, // Dark Orange
            5 => 0x9932CCff, // Dark Orchid
            _ => 0xFFFFFFff, // Fallback
        };

        for &(x, y) in &snake.positions {
            rect!(
                x = x * grid_size + 1,
                y = y * grid_size + 1,
                w = grid_size - 2,
                h = grid_size - 2,
                color = color
            );
        }
    }
}

fn draw_text(joined_game: bool) {
    if !joined_game {
        let [w, h] = canvas_size!();
        let msg = "Press SPACE to join";
        let font_w = 8;
        let font_h = 8;
        let len = msg.len() as u32;
        let x = (w / 2) - ((len * font_w) / 2);
        let y = (h / 2) - (font_h / 2);
        text!("Press SPACE to join", x = x, y = y, font = Font::L);
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
enum SnakeChannelMessage {
    StateUpdate(SnakeSession),
    PlayerJoined(),
    PlayerDied(),
}

#[export_name = "channel/snake_controller"]
unsafe extern "C" fn snake_controller() {
    let mut connected = BTreeSet::new(); // All Connected Players
    let mut driver = "".to_string(); // Only one player's messages will update the server
    let mut snake_id = 0;
    let mut player_snake_ids: BTreeMap<String, u8> = BTreeMap::new();
    let mut state = SnakeSession {
        grid_size: 16,
        snakes: Vec::new(),
        apples: Vec::new(),
    };

    loop {
        // Timeout runs every 128 ms.
        // When the TImeout runs all the snakes move and we check for overlaps
        match os::server::channel_recv_with_timeout(64) {
            // Handle a channel connection
            Ok(server::ChannelMessage::Connect(user_id, _data)) => {
                connected.insert(user_id.clone());
                // If we don't have a driver, then assign one
                if driver.is_empty() {
                    driver = user_id;
                }
            }
            // Handle a channel disconnection
            Ok(server::ChannelMessage::Disconnect(user_id, _data)) => {
                connected.remove(&user_id);
                if driver == user_id {
                    if let Some(next_driver) = connected.first() {
                        driver = next_driver.clone();
                    }
                }
                let snake_id = player_snake_ids.get(&user_id);
                if let Some(snake_id) = snake_id {
                    state.snakes.retain(|snake| snake.snake_id != *snake_id);
                };
                player_snake_ids.remove(&user_id);
            }

            // Handle custom message data sent to
            Ok(server::ChannelMessage::Data(user_id, data)) => {
                if let Ok(data) = PlayerMessage::try_from_slice(&data) {
                    match data {
                        PlayerMessage::JoinGame => {
                            // Check if you are already in the game
                            // If not, then create a snake and add it to the map
                            if !player_snake_ids.contains_key(&user_id) {
                                init_snake(&mut state.snakes, snake_id);
                                player_snake_ids.insert(user_id.clone(), snake_id);

                                let msg = SnakeChannelMessage::PlayerJoined();
                                os::server::channel_send(&user_id, &msg.try_to_vec().unwrap());
                                snake_id += 1;
                            }
                        }
                        PlayerMessage::ChangeDirection { dir } => {
                            let player_snake_id = player_snake_ids.get(&user_id);
                            let Some(player_snake_id) = player_snake_id else {
                                continue;
                            };
                            for snake in &mut state.snakes {
                                if snake.snake_id == *player_snake_id {
                                    snake.direction = match (snake.direction, dir) {
                                        (Direction::Up, Direction::Down) => snake.direction,
                                        (Direction::Down, Direction::Up) => snake.direction,
                                        (Direction::Left, Direction::Right) => snake.direction,
                                        (Direction::Right, Direction::Left) => snake.direction,
                                        _ => dir,
                                    };
                                    break;
                                }
                            }
                        }
                        PlayerMessage::ResetGame => {
                            state = SnakeSession {
                                grid_size: 16,
                                snakes: Vec::new(),
                                apples: Vec::new(),
                            };
                            player_snake_ids.clear();
                        }
                    }
                }
            }

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
            }
            // Handle a channel closure
            Err(err) => {
                os::server::log!("ERROR: {err:?}");
                return;
            }
        }
    }
}

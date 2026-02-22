use soroban_sdk::{contractclient, Address, Env};

/// Game Hub contract client interface.
/// In production, calls the Stellar Game Studio Game Hub at
/// CB4VZAT2U3UC6XFK3N23SKRF2NDCMP3QHJYMCHHFMZO7MRQO6DQ2EMYG.
/// For tests, use the mock in contracts/game-hub/.
#[contractclient(name = "GameHubClient")]
pub trait GameHub {
    fn start_game(
        env: Env,
        game_id: Address,
        session_id: u32,
        player1: Address,
        player2: Address,
        player1_points: i128,
        player2_points: i128,
    );

    fn end_game(env: Env, session_id: u32, player1_won: bool);
}

/// Notify the game hub that a new hand is starting.
pub fn notify_start(
    env: &Env,
    game_hub: &Address,
    game_id: &Address,
    session_id: u32,
    player1: &Address,
    player2: &Address,
    player1_points: i128,
    player2_points: i128,
) {
    let client = GameHubClient::new(env, game_hub);
    client.start_game(
        game_id,
        &session_id,
        player1,
        player2,
        &player1_points,
        &player2_points,
    );
}

/// Notify the game hub that a hand has ended.
pub fn notify_end(env: &Env, game_hub: &Address, session_id: u32, player1_won: bool) {
    let client = GameHubClient::new(env, game_hub);
    client.end_game(&session_id, &player1_won);
}

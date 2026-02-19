use soroban_sdk::{contract, contractclient, contractimpl, Address, Env};

/// Game hub contract interface.
/// Matches the interface at CB4VZAT2U3UC6XFK3N23SKRF2NDCMP3QHJYMCHHFMZO7MRQO6DQ2EMYG
#[contract]
#[allow(dead_code)]
pub struct GameHubContract;

#[allow(dead_code)]
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

/// Mock implementation for tests. In production, the real game hub
/// contract is deployed separately and called cross-contract.
#[contractimpl]
#[allow(dead_code)]
impl GameHubContract {
    pub fn start_game(
        _env: Env,
        _game_id: Address,
        _session_id: u32,
        _player1: Address,
        _player2: Address,
        _player1_points: i128,
        _player2_points: i128,
    ) {
        // No-op mock — real contract lives at CB4VZAT2U3UC6XFK3N23SKRF2NDCMP3QHJYMCHHFMZO7MRQO6DQ2EMYG
    }

    pub fn end_game(_env: Env, _session_id: u32, _player1_won: bool) {
        // No-op mock
    }
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

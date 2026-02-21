#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct GameHubContract;

#[contractimpl]
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
        // Stub: accepts call without doing anything.
    }

    pub fn end_game(_env: Env, _session_id: u32, _player1_won: bool) {
        // Stub: accepts call without doing anything.
    }
}

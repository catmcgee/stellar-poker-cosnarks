use soroban_sdk::{contracttype, Address, BytesN, Vec};

#[contracttype]
#[derive(Clone, Debug)]
pub struct TableConfig {
    pub token: Address,           // Payment token (e.g., USDC)
    pub min_buy_in: i128,
    pub max_buy_in: i128,
    pub small_blind: i128,
    pub big_blind: i128,
    pub max_players: u32,         // 2-9
    pub timeout_ledgers: u32,     // Ledgers before timeout (~5 sec each)
    pub committee: Address,       // MPC committee address
    pub verifier: Address,        // ZK verifier contract address
    pub game_hub: Address,        // Game hub contract for start_game/end_game
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PlayerState {
    pub address: Address,
    pub stack: i128,
    pub bet_this_round: i128,
    pub folded: bool,
    pub all_in: bool,
    pub sitting_out: bool,
    pub seat_index: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum GamePhase {
    Waiting,        // Waiting for players
    Dealing,        // Committee is dealing
    Preflop,        // Betting round: preflop
    DealingFlop,    // Committee revealing flop
    Flop,           // Betting round: flop
    DealingTurn,    // Committee revealing turn
    Turn,           // Betting round: turn
    DealingRiver,   // Committee revealing river
    River,          // Betting round: river
    Showdown,       // Revealing hands and determining winner
    Settlement,     // Pot distributed, ready for next hand
    Dispute,        // Something went wrong; funds frozen
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum Action {
    Fold,
    Check,
    Call,
    Bet(i128),
    Raise(i128),
    AllIn,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SidePot {
    pub amount: i128,
    pub eligible_players: Vec<u32>, // seat indices
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TableState {
    pub id: u32,
    pub admin: Address,
    pub config: TableConfig,
    pub phase: GamePhase,
    pub players: Vec<PlayerState>,
    pub dealer_seat: u32,
    pub current_turn: u32,
    pub pot: i128,
    pub side_pots: Vec<SidePot>,
    pub deck_root: BytesN<32>,
    pub hand_commitments: Vec<BytesN<32>>,
    pub board_cards: Vec<u32>,        // Revealed community cards
    pub dealt_indices: Vec<u32>,      // Deck indices already dealt
    pub hand_number: u32,
    pub last_action_ledger: u32,      // For timeout calculation
    pub committee: Address,
    pub session_id: u32,              // Game hub session ID for current hand
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Table(u32),
}

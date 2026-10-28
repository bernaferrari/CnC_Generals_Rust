use game_engine::common::system::xfer_save::XferSave;
use generals_main::game_logic::{GameLogic, Player, Team};
use generals_main::save_load::snapshot::{
    PlayerTeamChunks, apply_pending_player_team_chunks, stamp_player_team_chunks,
    stash_loaded_player_team_chunks, write_players_block, write_team_factory_block,
};
use std::io::Cursor;

fn encode_and_decode(chunks: &PlayerTeamChunks) -> PlayerTeamChunks {
    let mut players = Cursor::new(Vec::<u8>::new());
    {
        let mut xfer = XferSave::new(&mut players, 1);
        write_players_block(&mut xfer, chunks).expect("encode Players");
    }
    let mut teams = Cursor::new(Vec::<u8>::new());
    {
        let mut xfer = XferSave::new(&mut teams, 1);
        write_team_factory_block(&mut xfer, chunks).expect("encode TeamFactory");
    }
    stash_loaded_player_team_chunks(Some(&players.into_inner()), Some(&teams.into_inner()))
        .expect("decode Players and TeamFactory")
}

fn world_with_player() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world
}

#[test]
fn encoded_player_default_team_identity_round_trips_and_rejects_bad_team_id() {
    let source = world_with_player();
    let captured = stamp_player_team_chunks(&source).expect("capture session chunks");
    let saved_id = captured.players.as_ref().unwrap().players[0]
        .host_default_team_id
        .expect("admitted Player default Team");
    let decoded = encode_and_decode(&captured);
    assert_eq!(
        decoded.players.as_ref().unwrap().players[0].host_default_team_id,
        Some(saved_id)
    );

    let mut receiving = world_with_player();
    apply_pending_player_team_chunks(&mut receiving, &decoded).expect("apply encoded chunks");
    let restored = stamp_player_team_chunks(&receiving).expect("capture restored session");
    assert_eq!(
        restored.players.as_ref().unwrap().players[0].host_default_team_id,
        Some(saved_id)
    );

    let mut malformed = captured;
    malformed.players.as_mut().unwrap().players[0].host_default_team_id = Some(u32::MAX);
    let malformed = encode_and_decode(&malformed);
    let mut receiving = world_with_player();
    assert!(apply_pending_player_team_chunks(&mut receiving, &malformed).is_err());
}

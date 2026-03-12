// FILE: GameSpyChat.h //////////////////////////////////////////////////////
// Generals GameSpy Chat
// Author: Matthew D. Campbell, February 2002

#pragma once

#ifndef __GAMESPYCHAT_H__
#define __GAMESPYCHAT_H__

#include "GameSpy/Peer/Peer.h"

class GameWindow;
class WindowLayout;

Bool GameSpySendChat(UnicodeString message, Bool isEmote, GameWindow *playerListbox = NULL);
void GameSpyAddText( UnicodeString message, GameSpyColors color = GSCOLOR_DEFAULT );

extern GameWindow *progressTextWindow;				///< Text box on the progress screen
extern GameWindow *quickmatchTextWindow;			///< Text box on the quickmatch screen
extern GameWindow *quickmatchTextWindow;			///< Text box on the quickmatch screen
extern GameWindow *listboxLobbyChat;					///< Chat box on the custom lobby screen
extern GameWindow *listboxLobbyPlayers;				///< Player box on the custom lobby screen
extern GameWindow *listboxLobbyGames;					///< Game box on the custom lobby screen
extern GameWindow *listboxLobbyChatChannels;	///< Chat channel box on the custom lobby screen
extern GameWindow *listboxGameSetupChat;			///< Chat box on the custom game setup screen
extern WindowLayout *WOLMapSelectLayout;			///< Map selection overlay

void RoomMessageCallback(PEER peer, RoomType roomType,
												 const char * nick, const char * message,
												 MessageType messageType, void * param);		///< Called when a message arrives in a room.

void PlayerMessageCallback(PEER peer, const char * nick,
													 const char * message, MessageType messageType,
													 void * param);														///< Called when a private message is received from another player.

#endif // __GAMESPYCHAT_H__

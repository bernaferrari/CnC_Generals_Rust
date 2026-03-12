#pragma once

#ifndef __DISCONNECTDIALOG_H
#define __DISCONNECTDIALOG_H

#include "GameNetwork/DisconnectManager.h"

enum DisconnectMenuStateType {
	DISCONNECTMENUSTATETYPE_SCREENON,
	DISCONNECTMENUSTATETYPE_SCREENOFF
};

class DisconnectMenu {
public:
	DisconnectMenu();
	virtual ~DisconnectMenu();

	void init();

	void attachDisconnectManager(DisconnectManager *disconnectManager);

	void showScreen();
	void hideScreen();
	Bool isScreenVisible( void ) { return m_menuState == DISCONNECTMENUSTATETYPE_SCREENON; }

	void showPlayerControls(Int slot);
	void hidePlayerControls(Int slot);
	void showPacketRouterTimeout();
	void hidePacketRouterTimeout();

	void setPlayerName(Int playerNum, UnicodeString name);
	void setPlayerTimeoutTime(Int playerNum, time_t newTime);
	void setPacketRouterTimeoutTime(time_t newTime);

	void sendChat(UnicodeString text);
	void showChat(UnicodeString text);

	void quitGame();
	void removePlayer(Int slot, UnicodeString playerName);
	void voteForPlayer(Int slot);
	void updateVotes(Int slot, Int votes);

protected:
	DisconnectManager *m_disconnectManager;		///< For retrieving status updates from the disconnect manager.
	DisconnectMenuStateType m_menuState;			///< The current state of the menu screen.

	static char *m_playerNameTextControlNames[MAX_SLOTS];	///< names of the player name controls in the window.
	static char *m_playerTimeoutTextControlNames[MAX_SLOTS]; ///< names of the timeout controls in the window.
	static char *m_playerVoteButtonControlNames[MAX_SLOTS];	///< names of the vote button controls in the window.
	static char *m_playerVoteCountControlNames[MAX_SLOTS];	///< names of the vote count static text controls in the window.
	static char *m_packetRouterTimeoutControlName;	///< name of the packet router timeout control window.
	static char *m_packetRouterTimeoutLabelControlName; ///< name of the packet router timeout label control window.
	static char *m_textDisplayControlName;	///< name of the text display listbox control window.
};

extern DisconnectMenu *TheDisconnectMenu;

#endif // #ifndef __DISCONNECTDIALOG_H

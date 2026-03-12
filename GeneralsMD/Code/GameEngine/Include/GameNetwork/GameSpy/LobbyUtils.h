// FILE: LobbyUtils.h //////////////////////////////////////////////////////
// Generals lobby utils
// Author: Matthew D. Campbell, Sept 2002

#pragma once

#ifndef __LOBBYUTILS_H__
#define __LOBBYUTILS_H__

class GameWindow;

GameWindow *GetGameListBox( void );
GameWindow *GetGameInfoListBox( void );
NameKeyType GetGameListBoxID( void );
NameKeyType GetGameInfoListBoxID( void );
void GrabWindowInfo( void );
void ReleaseWindowInfo( void );
void RefreshGameInfoListBox( GameWindow *mainWin, GameWindow *win );
void RefreshGameListBoxes( void );
void ToggleGameListType( void );

void playerTemplateComboBoxTooltip(GameWindow *wndComboBox, WinInstanceData *instData, UnsignedInt mouse);
void playerTemplateListBoxTooltip(GameWindow *wndListBox, WinInstanceData *instData, UnsignedInt mouse);

enum GameSortType
{
	GAMESORT_ALPHA_ASCENDING = 0,
	GAMESORT_ALPHA_DESCENDING,
	GAMESORT_PING_ASCENDING,
	GAMESORT_PING_DESCENDING,
	GAMESORT_MAX,
};

Bool HandleSortButton( NameKeyType sortButton );
void PopulateLobbyPlayerListbox(void);

#endif // __LOBBYUTILS_H__

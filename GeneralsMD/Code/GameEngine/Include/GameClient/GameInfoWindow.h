// FILE: GameInfoWindow.h ///////////////////////////////////////////////////////////////////////////
// Created:    Chris Huybregts, Feb 2002
// Desc:       Game Info Window Header
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GAMEINFOWINDOW_H_
#define __GAMEINFOWINDOW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameClient/GameWindow.h"
#include "GameNetwork/LANGameInfo.h"

// Function Stubs for GameInfoWindow
extern void CreateLANGameInfoWindow( GameWindow *sizeAndPosWin );
extern void DestroyGameInfoWindow(void);
extern void RefreshGameInfoWindow(GameInfo *gameInfo, UnicodeString gameName);
extern void HideGameInfoWindow(Bool hide);

#endif // __GAMEINFOWINDOW_H_


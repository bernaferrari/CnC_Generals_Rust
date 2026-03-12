// FILE: GamespyOverlay.h //////////////////////////////////////////////////////
// Generals GameSpy overlay screens
// Author: Matthew D. Campbell, March 2002

#pragma once

#ifndef __GAMESPYOVERLAY_H__
#define __GAMESPYOVERLAY_H__

#include "Common/NameKeyGenerator.h"
#include "GameClient/WindowLayout.h"
#include "GameClient/Gadget.h"
#include "GameClient/Shell.h"
#include "GameClient/KeyDefs.h"
#include "GameClient/GameWindowManager.h"

void ClearGSMessageBoxes( void );	///< Tear down any GS message boxes (e.g. in case we have a new one to put up)
void GSMessageBoxOk(UnicodeString titleString,UnicodeString bodyString, GameWinMsgBoxFunc okFunc = NULL);	///< Display a Message box with Ok button and track it
void GSMessageBoxOkCancel(UnicodeString title, UnicodeString message, GameWinMsgBoxFunc okFunc, GameWinMsgBoxFunc cancelFunc);	///< Display a Message box with Ok/Cancel buttons and track it
void GSMessageBoxYesNo(UnicodeString title, UnicodeString message, GameWinMsgBoxFunc yesFunc, GameWinMsgBoxFunc noFunc);	///< Display a Message box with Yes/No buttons and track it
void RaiseGSMessageBox( void );		///< Bring GS message box to the foreground (if we transition screens while a message box is up)

enum GSOverlayType
{
	GSOVERLAY_PLAYERINFO,
	GSOVERLAY_MAPSELECT,
	GSOVERLAY_BUDDY,
	GSOVERLAY_PAGE,
	GSOVERLAY_GAMEOPTIONS,
	GSOVERLAY_GAMEPASSWORD,
	GSOVERLAY_LADDERSELECT,
	GSOVERLAY_LOCALESELECT,
	GSOVERLAY_OPTIONS,
	GSOVERLAY_MAX
};

void GameSpyOpenOverlay( GSOverlayType );
void GameSpyCloseOverlay( GSOverlayType );
void GameSpyCloseAllOverlays( void );
Bool GameSpyIsOverlayOpen( GSOverlayType );
void GameSpyToggleOverlay( GSOverlayType );
void GameSpyUpdateOverlays( void );
void ReOpenPlayerInfo( void );
void CheckReOpenPlayerInfo(void );
#endif // __GAMESPYOVERLAY_H__

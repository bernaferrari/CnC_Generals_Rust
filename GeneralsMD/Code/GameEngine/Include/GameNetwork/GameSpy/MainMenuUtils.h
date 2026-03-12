// FILE: MainMenuUtils.h //////////////////////////////////////////////////////
// Author: Matthew D. Campbell, Sept 2002
// Description: GameSpy version check, patch download, etc utils
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __MAINMENUUTILS_H__
#define __MAINMENUUTILS_H__

void HTTPThinkWrapper( void );
void StopAsyncDNSCheck( void );
void StartPatchCheck( void );
void CancelPatchCheckCallback( void );
void StartDownloadingPatches( void );
void HandleCanceledDownload( Bool resetDropDown = TRUE );

void CheckOverallStats( void );
void HandleOverallStats( const char* szHTTPStats, unsigned len );

void CheckNumPlayersOnline( void );
void HandleNumPlayersOnline( Int numPlayersOnline );

#endif // __MAINMENUUTILS_H__

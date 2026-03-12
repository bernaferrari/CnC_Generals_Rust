//
// Project:    RTS 3
//
// File name:  GameClient/GameText.h
//
// Created:    11/07/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __GAMECLIENT_GAMETEXT_H_
#define __GAMECLIENT_GAMETEXT_H_


//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------

//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// GameTextInterface 
//===============================
/** Game text interface object for localised text.
	*/
//===============================

class GameTextInterface
{

	public:

		virtual ~GameTextInterface() {};

		virtual void					init( void ) = 0;						///< Initlaizes the text system
		virtual const wchar_t * fetch( const char *label ) = 0;		///< Returns the associated labeled unicode text

};


extern GameTextInterface *TheGameText;
extern GameTextInterface* CreateGameTextInterface( void );

//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------


#endif // __GAMECLIENT_GAMETEXT_H_

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

class AsciiString;
class UnicodeString;

//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------
typedef std::vector<AsciiString> AsciiStringVec;

//===============================
// GameTextInterface 
//===============================
/** Game text interface object for localised text.
	*/
//===============================

class GameTextInterface : public SubsystemInterface
{

	public:

		virtual ~GameTextInterface() {};

		virtual UnicodeString fetch( const Char *label, Bool *exists = NULL ) = 0;		///< Returns the associated labeled unicode text
		virtual UnicodeString fetch( AsciiString label, Bool *exists = NULL ) = 0;		///< Returns the associated labeled unicode text
		// This function is not performance tuned.. Its really only for Worldbuilder. jkmcd
		virtual AsciiStringVec& getStringsWithLabelPrefix(AsciiString label) = 0;

		virtual void					initMapStringFile( const AsciiString& filename ) = 0;
};


extern GameTextInterface *TheGameText;
extern GameTextInterface* CreateGameTextInterface( void );

//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------


#endif // __GAMECLIENT_GAMETEXT_H_

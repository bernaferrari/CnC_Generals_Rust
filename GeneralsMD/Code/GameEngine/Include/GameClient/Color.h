// FILE: Color.h //////////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  Color.h
//
// Created:    Colin Day, July 2001
//
// Desc:       Management of color representations
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __COLOR_H_
#define __COLOR_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "Lib/BaseType.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////
enum { GAME_COLOR_UNDEFINED = 0x00FFFFFF }; // this is white with zero alpha... safe to use!

/** @todo we need real color representation, this is just palce holder so we
can more easily identify sections of the code that need it */
typedef Int Color;

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////

inline Color GameMakeColor( UnsignedByte red, UnsignedByte green, UnsignedByte blue, UnsignedByte alpha )
{
	return (alpha << 24) | (red << 16) | (green << 8) | (blue); 
}

extern void GameGetColorComponents( Color color,
																	  UnsignedByte *red,
																	  UnsignedByte *green,
																	  UnsignedByte *blue,
																	  UnsignedByte *alpha );

// Put on ice until later - M Lorenzen
//extern void GameGetColorComponentsWithCheatSpy( Color color,
//																	  UnsignedByte *red,
//																	  UnsignedByte *green,
//																	  UnsignedByte *blue,
//																	  UnsignedByte *alpha );


extern void GameGetColorComponentsReal( Color color, Real *red, Real *green, Real *blue, Real *alpha );

extern Color GameDarkenColor( Color color, Int percent = 10 );

#endif // __COLOR_H_


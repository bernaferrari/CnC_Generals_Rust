// FILE: W3DGameFont.h ////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  W3DGameFont.h
//
// Created:    Colin Day, June 2001
//
// Desc:       W3D implementation for managing font definitions
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DGAMEFONT_H_
#define __W3DGAMEFONT_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameClient/GameFont.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////

// W3DFontLibrary -------------------------------------------------------------
/** Our font library that uses W3D font implementations */
//-----------------------------------------------------------------------------
class W3DFontLibrary : public FontLibrary
{

public:

	W3DFontLibrary( void ) { }
	~W3DFontLibrary( void ) { }

protected:

	/// load the font data pointer based on everything else we already have set
	Bool loadFontData( GameFont *font );
	/// release the font data pointer
	void releaseFontData( GameFont *font );

};  // end W3DFontLibrary

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////

#endif // __W3DGAMEFONT_H_


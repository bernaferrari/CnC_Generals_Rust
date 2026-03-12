// FILE: DisplayString.h //////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  DisplayString.h
//
// Created:    Colin Day, July 2001
//
// Desc:       Contstuct for holding double byte game string data and being
//						 able to draw that text to the screen.
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DISPLAYSTRING_H_
#define __DISPLAYSTRING_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "Lib/BaseType.h"
#include "GameClient/GameFont.h"
#include "GameClient/Color.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/GameMemory.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////
class DisplayStringManager;

// TYPE DEFINES ///////////////////////////////////////////////////////////////

// DisplayString --------------------------------------------------------------
/** String representation that can also has additional information and
	* methods for drawing to the screen */
//-----------------------------------------------------------------------------
class DisplayString : public MemoryPoolObject
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DisplayString, "DisplayString" )

public:

	friend DisplayStringManager;

	DisplayString( void );
	// virtual ~DisplayString( void );  // destructor defined by memory pool

	virtual void setText( UnicodeString text );		///< set text for this string
	virtual UnicodeString getText( void );				///< get text for this string
	virtual Int getTextLength( void );				///< return number of chars in string
	virtual void notifyTextChanged( void );		///< called when text has changed
	virtual void reset( void );								///< reset all contents of string

	virtual void setFont( GameFont *font );		///< set a font for display
	virtual GameFont *getFont( void );				///< return font in string
	virtual void setWordWrap( Int wordWrap ) = 0;	///< Set the width that we want to start wrapping text
	virtual void setWordWrapCentered( Bool isCentered ) = 0; ///< If this is set to true, the text on a new line is centered
	virtual void draw( Int x, Int y, Color color, Color dropColor ) = 0;  ///< render text
	virtual void draw( Int x, Int y, Color color, Color dropColor, Int xDrop, Int yDrop ) = 0;  ///< render text with the drop shadow being at the offsets passed in
	virtual void getSize( Int *width, Int *height ) = 0;  ///< get render size
	virtual Int getWidth( Int charPos = -1 ) = 0; ///< get text with up to charPos characters, 1- = all characters

	virtual void setUseHotkey( Bool useHotkey, Color hotKeyColor ) = 0;

	virtual void setClipRegion( IRegion2D *region );  ///< clip text in this region

	virtual void removeLastChar( void );			///< remove the last character
	virtual void appendChar( WideChar c );		///< append character to end

	DisplayString *next( void );							///< return next string

protected:

	UnicodeString m_textString;
	GameFont *m_font;			 ///< font to display this string with
	
	DisplayString *m_next;  ///< for the display string factory list ONLY
	DisplayString *m_prev;	///< for the display string factory list ONLY

};  // end DisplayString

///////////////////////////////////////////////////////////////////////////////
// INLINING ///////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////
inline UnicodeString DisplayString::getText( void ) { return m_textString; }
inline Int DisplayString::getTextLength( void ) { return m_textString.getLength(); }
inline void DisplayString::setFont( GameFont *font ) { m_font = font; }
inline GameFont *DisplayString::getFont( void ) { return m_font; }
inline void DisplayString::setClipRegion( IRegion2D *region ) {}
inline void DisplayString::notifyTextChanged( void ) {}
inline DisplayString *DisplayString::next( void ) { return m_next; }

// EXTERNALS //////////////////////////////////////////////////////////////////

#endif // __DISPLAYSTRING_H_


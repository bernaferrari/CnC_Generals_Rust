// FILE: W3DGameWindow.h //////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  W3DGameWindow.h
//
// Created:    Colin Day, June 2001
//
// Desc:       W3D implemenations for the game windowing system
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DGAMEWINDOW_H_
#define __W3DGAMEWINDOW_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameClient/GameWindow.h"
#include "WW3D2/Render2DSentence.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////

// W3DGameWindow --------------------------------------------------------------
/** W3D implemenation for a game window */
// ----------------------------------------------------------------------------
class W3DGameWindow : public GameWindow
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(W3DGameWindow, "W3DGameWindow")		

public:

	W3DGameWindow( void );
	// already defined by MPO.
	//~W3DGameWindow( void );

	/// draw borders for this window only, NO child windows or anything else
	void winDrawBorder( void );

	Int winSetPosition( Int x, Int y );  ///< set window position
	Int winSetText( UnicodeString newText );  ///< set text string	
	void winSetFont( GameFont *font );  ///< set font for window

	void getTextSize( Int *width, Int *height );  ///< get size of text
	void setTextLoc( Int x, Int y );  ///< set text screen coord loc
	void drawText( Color color );  ///< draw text in the text renderer
		
protected:

	/// helper function to draw borders
	void blitBorderRect( Int x, Int y, Int width, Int height );

	Render2DSentenceClass m_textRenderer;  ///< for drawing text
	ICoord2D m_textPos;  ///< current text pos set in text renderer
	Color m_currTextColor;  ///< current color used in text renderer
	Bool m_needPolyDraw;  ///< TRUE need to redo the text polys
	Bool m_newTextPos;  ///< TRUE when our window has moved and we need a new text pos

};  // end class W3DGameWindow

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////
extern void W3DGameWinDefaultDraw( GameWindow *window, 
																	 WinInstanceData *instData );

#endif // __W3DGAMEWINDOW_H_


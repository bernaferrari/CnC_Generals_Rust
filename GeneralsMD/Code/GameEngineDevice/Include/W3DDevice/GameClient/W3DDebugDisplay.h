//
// Project:    Generals
//
// Module:     Debug
//
// File name:  W3DDevice/GameClient/W3DDebugDisplay.h
//
// Created:    11/13/01 TR
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __W3DDEVICE_GAMECLIENT_W3DDEBUGDISPLAY_H
#define __W3DDEVICE_GAMECLIENT_W3DDEBUGDISPLAY_H


//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "GameClient/DebugDisplay.h"


//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------

class GameFont;
class DisplayString;

//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------


//===============================
// W3DDebugDisplay 
//===============================

class W3DDebugDisplay : public DebugDisplay
{

	public:

		W3DDebugDisplay();
		virtual ~W3DDebugDisplay();

		void init( void );																						///< Initialized the display
		void setFont( GameFont *font );																///< Set the font to render with
		void setFontWidth( Int width ) { m_fontWidth = width; };			///< Set the font width
		void setFontHeight( Int height ) { m_fontHeight = height; };		///< Set the font height

	protected:

		GameFont *m_font;			///< Font to render text with
		Int m_fontWidth;
		Int m_fontHeight;
		DisplayString *m_displayString;

		virtual void drawText( Int x, Int y, Char *text );			///< Render null ternimated string at current cursor position

};


//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------



#endif // __W3DDEVICE_GAMECLIENT_W3DDEBUGDISPLAY_H

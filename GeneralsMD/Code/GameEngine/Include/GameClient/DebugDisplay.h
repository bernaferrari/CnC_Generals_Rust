//
// Project:    Generals
//
// Module:     Debug
//
// File name:  GameClient/DebugDisplay.h
//
// Created:    11/13/01 TR
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __GAMECLIENT_DEBUGDISPLAY_H
#define __GAMECLIENT_DEBUGDISPLAY_H


//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "Lib/BaseType.h"
#include <stdio.h>


//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------


//===============================
// DebugDisplayInterface 
//===============================

class DebugDisplayInterface
{

	public:

		enum Color
		{
			WHITE,
			BLACK,
			YELLOW,
			RED,
			GREEN,
			BLUE,
			NUM_COLORS
		};

		virtual ~DebugDisplayInterface() {};

		virtual void	printf( Char *format, ...) = 0;			///< Print formatted text at current cursor position
		virtual void	setCursorPos( Int x, Int y ) = 0;		///< Set new cursor position
		virtual Int		getCursorXPos( void ) = 0;					///< Get current X position of cursor
		virtual Int		getCursorYPos( void ) = 0;					///< Get current Y position of cursor
		virtual Int		getWidth( void ) = 0;								///< Get character width of display
		virtual Int		getHeight( void ) = 0;							///< Get character height of display
		virtual void	setTextColor( Color color ) = 0;		///< Set text color
		virtual void	setRightMargin( Int rightPos ) = 0;	///< Set right margin position
		virtual void	setLeftMargin( Int leftPos ) = 0;		///< Set left margin position
		virtual void	reset( void ) = 0;									///< Reset back to default settings

	protected:

		virtual void drawText( Int x, Int y, Char *text ) = 0;			///< Render null ternimated string at current cursor position
};


//===============================
// DebugDisplay 
//===============================

class DebugDisplay : public DebugDisplayInterface
{

	public:

		DebugDisplay();
		virtual ~DebugDisplay() {};

		virtual void	printf( Char *format, ...);			///< Print formatted text at current cursor position
		virtual void	setCursorPos( Int x, Int y );		///< Set new cursor position
		virtual Int		getCursorXPos( void );					///< Get current X position of cursor
		virtual Int		getCursorYPos( void );					///< Get current Y position of cursor
		virtual Int		getWidth( void );								///< Get character width of display
		virtual Int		getHeight( void );							///< Get character height of display
		virtual void	setTextColor( Color color );		///< set text color
		virtual void	setRightMargin( Int rightPos );	///< set right margin position
		virtual void	setLeftMargin( Int leftPos );		///< set left margin position
		virtual void	reset( void );									///< Reset back to default settings

	protected:

		Color m_textColor;	///< Color to render text in
		Int		m_xPos;				///< Current X position of cursor
		Int		m_yPos;				///< Current Y position of cursor
		Int		m_width;			///< Character width of display
		Int		m_height;			///< Character height of display
		Int		m_rightMargin;///< Right margin position
		Int		m_leftMargin;	///< Left margin position

};

// displayers

#if defined(_DEBUG) || defined(_INTERNAL)
extern void AudioDebugDisplay( DebugDisplayInterface *debugDisplay, void *userData, FILE *fp );
#endif


//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------



#endif // __GAMECLIENT_DEBUGDISPLAY_H

// FILE: DisplayString.cpp ////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  DisplayString.cpp
//
// Created:    Colin Day, July 2001
//
// Desc:       Contstuct for holding double byte game string data and being
//						 able to draw that text to the screen.
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "Common/Debug.h"
#include "Common/Language.h"
#include "GameClient/DisplayString.h"

// DEFINES ////////////////////////////////////////////////////////////////////

// PRIVATE TYPES //////////////////////////////////////////////////////////////

// PRIVATE DATA ///////////////////////////////////////////////////////////////

// PUBLIC DATA ////////////////////////////////////////////////////////////////

// PRIVATE PROTOTYPES /////////////////////////////////////////////////////////

// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// DisplayString::DisplayString ===============================================
/** */
//=============================================================================
DisplayString::DisplayString( void )
{
	// m_textString = "";	// not necessary, done by default
	m_font = NULL;

	m_next = NULL;
	m_prev = NULL;

}  // end DisplayString

// DisplayString::~DisplayString ==============================================
/** */
//=============================================================================
DisplayString::~DisplayString( void )
{

	// free any data
	reset();

}  // end ~DisplayString

// DisplayString::setText =====================================================
/** Copy the text to this instance */
//=============================================================================
void DisplayString::setText( UnicodeString text )
{
	if (text == m_textString) 
		return;

	m_textString = text;

	// our text has now changed
	notifyTextChanged();

}  // end setText

// DisplayString::reset =======================================================
/** Free and reset all the data for this string, effectively making this
	* instance like brand new */
//=============================================================================
void DisplayString::reset( void )
{

	m_textString.clear();

	// no font
	m_font = NULL;

}  // end reset

// DisplayString::removeLastChar ==============================================
/** Remove the last character from the string text */
//=============================================================================
void DisplayString::removeLastChar( void )
{
	m_textString.removeLastChar();

	// our text has now changed
	notifyTextChanged();

}  // end removeLastChar

// DisplayString::appendChar ==================================================
/** Append character to the end of the string */
//=============================================================================
void DisplayString::appendChar( WideChar c )
{
	m_textString.concat(c);

	// text has now changed
	notifyTextChanged();

}  // end appendchar


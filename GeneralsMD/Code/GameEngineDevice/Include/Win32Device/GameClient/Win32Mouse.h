// FILE: Win32Mouse.h /////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  Win32Mouse.h
//
// Created:    Colin Day, July 2001
//
// Desc:       Interface for the mouse using only the Win32 messages
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __WIN32MOUSE_H_
#define __WIN32MOUSE_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameClient/Mouse.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////
enum { NO_TIME_FROM_WINDOWS = 0 };
// Win32Mouse -----------------------------------------------------------------
/** Mouse interface for when using only the Win32 messages */
//-----------------------------------------------------------------------------
class Win32Mouse : public Mouse
{

public:

	Win32Mouse( void );
	virtual ~Win32Mouse( void );

	virtual void init( void );		///< init mouse, extend this functionality, do not replace
	virtual void reset( void );		///< reset the system
	virtual void update( void );	///< update
	virtual void initCursorResources(void);	///< load windows resources needed for 2d cursors.

	virtual void setCursor( MouseCursor cursor );		///< set mouse cursor
	virtual void capture( void );										///< capture the mouse
	virtual void releaseCapture( void );						///< release mouse capture

	virtual void setVisibility(Bool visible);

	/// add an event from a win32 window procedure
	void addWin32Event( UINT msg, WPARAM wParam, LPARAM lParam, DWORD time );
	void lostFocus (Bool state) { m_lostFocus = state;}

protected:

	/// get the next event available in the buffer
	virtual UnsignedByte getMouseEvent( MouseIO *result, Bool flush );

	/// translate a win32 mouse event to our own info
	void translateEvent( UnsignedInt eventIndex, MouseIO *result );

	struct Win32MouseEvent
	{
		UINT msg;				///< WM_* message
		WPARAM wParam;  ///< WPARAM from the WM_* message
		LPARAM lParam;  ///< LPARAM from the WM_* message
		DWORD time;			///< TIME from the WM_* message
	};
	/// this is our buffer of events that we receive via a WndProc message
	Win32MouseEvent m_eventBuffer[ Mouse::NUM_MOUSE_EVENTS ];
	UnsignedInt m_nextFreeIndex;  ///< insert new events at this index
	UnsignedInt m_nextGetIndex;  /** events retrieved through getMouseEvent
															 will come from this index, then it will be
															 incremented to the next index */
	MouseCursor m_currentWin32Cursor;	///< keep track of last cursor image sent to D3D.
	Int m_directionFrame;	///< current frame of directional cursor (frome 0 points up).
	Bool m_lostFocus;		///< flag if window has lost focues and mouse should stop being updated.	
};  // end Win32Mouse 

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////

#endif // __WIN32MOUSE_H_


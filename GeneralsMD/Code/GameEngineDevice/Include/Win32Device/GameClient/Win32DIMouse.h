// FILE: Win32DIMouse.h ///////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  Win32DIMouse.h
//
// Created:    Colin Day, June 2001
//
// Desc:       Win32 direct input implementation for the mouse
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __WIN32DIMOUSE_H_
#define __WIN32DIMOUSE_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
#ifndef DIRECTINPUT_VERSION
#	define DIRECTINPUT_VERSION	0x800
#endif

#include <dinput.h>

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameClient/Mouse.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////

// class DirectInputMouse -----------------------------------------------------
/** Direct input implementation for the mouse device */
//-----------------------------------------------------------------------------
class DirectInputMouse : public Mouse
{

public:

	DirectInputMouse( void );
	virtual ~DirectInputMouse( void );

	// extended methods from base class
	virtual void init( void );		///< initialize the direct input mouse, extending functionality
	virtual void reset( void );		///< reset system
	virtual void update( void );  ///< update the mouse data, extending functionality
	virtual void setPosition( Int x, Int y );  ///< set position for mouse

	virtual void setMouseLimits( void );  ///< update the limit extents the mouse can move in

	virtual void setCursor( MouseCursor cursor );  ///< set mouse cursor

	virtual void capture( void );  ///< capture the mouse
	virtual void releaseCapture( void );  ///< release mouse capture
		
protected:

	/// device implementation to get mouse event
	virtual UnsignedByte getMouseEvent( MouseIO *result, Bool flush );

	// new internal methods for our direct input implemetation
	void openMouse( void );  ///< create the direct input mouse 
	void closeMouse( void );  ///< close and release mouse resources
	/// map direct input mouse data to our own format
	void mapDirectInputMouse( MouseIO *mouse, DIDEVICEOBJECTDATA *mdat );

	// internal data members for our direct input mouse
	LPDIRECTINPUT8 m_pDirectInput;  ///< pointer to direct input interface
	LPDIRECTINPUTDEVICE8 m_pMouseDevice;  ///< pointer to mouse device

};  // end class DirectInputMouse

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////

#endif // __WIN32DIMOUSE_H_


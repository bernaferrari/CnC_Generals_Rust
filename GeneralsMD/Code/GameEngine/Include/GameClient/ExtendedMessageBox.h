// FILE: ExtendedMessageBox.h /////////////////////////////////////////////////////////////////////
// Author: Matt Campbell, January 2003
// Description: We go quiet in 1 day, gold in 15.  Poor time to rewrite message boxes, so
//              we get this file instead.  Phooey.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __EXTENDEDMESSAGEBOX_H_
#define __EXTENDEDMESSAGEBOX_H_

#include "GameClient/GameWindowManager.h"

// return codes for message box callbacks
enum MessageBoxReturnType {
	MB_RETURN_CLOSE,
	MB_RETURN_KEEPOPEN
};

typedef MessageBoxReturnType (* MessageBoxFunc)( void *userData );

// WindowExMessageBoxData ---------------------------------------------------------
/** Data attached to each extended Message box window */
//-----------------------------------------------------------------------------
struct WindowExMessageBoxData
{
	MessageBoxFunc yesCallback;		///<Function pointer to the Yes Button Callback
  MessageBoxFunc noCallback;			///<Function pointer to the No Button Callback
  MessageBoxFunc okCallback;			///<Function pointer to the Ok Button Callback
  MessageBoxFunc cancelCallback;	///<Function pointer to the Cancel Button Callback
	void *userData;
};


GameWindow *ExMessageBoxYesNo				(UnicodeString titleString,UnicodeString bodyString, void *userData,
																		 MessageBoxFunc yesCallback, MessageBoxFunc noCallback);

GameWindow *ExMessageBoxYesNoCancel	(UnicodeString titleString,UnicodeString bodyString, void *userData,
																		 MessageBoxFunc yesCallback, MessageBoxFunc noCallback, MessageBoxFunc cancelCallback);

GameWindow *ExMessageBoxOkCancel		(UnicodeString titleString,UnicodeString bodyString, void *userData,
																		 MessageBoxFunc okCallback, MessageBoxFunc cancelCallback);

GameWindow *ExMessageBoxOk					(UnicodeString titleString,UnicodeString bodyString, void *userData,
																		 MessageBoxFunc okCallback);

GameWindow *ExMessageBoxCancel			(UnicodeString titleString,UnicodeString bodyString, void *userData,
																		 MessageBoxFunc cancelCallback);

#endif //__EXTENDEDMESSAGEBOX_H_
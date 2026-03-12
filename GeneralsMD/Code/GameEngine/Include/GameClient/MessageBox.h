// FILE: MessageBox.h /////////////////////////////////////////////////////////////////////////////
// Author: Chris Huybregts, November 2001
// Description: Message Box file containing user aliases
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __MESSAGEBOX_H_
#define __MESSAGEBOX_H_

#include "GameClient/GameWindowManager.h"

GameWindow *MessageBoxYesNo(UnicodeString titleString,UnicodeString bodyString,GameWinMsgBoxFunc yesCallback,GameWinMsgBoxFunc noCallback);  ///< convenience function for displaying a Message box with Yes and No buttons
GameWindow *QuitMessageBoxYesNo(UnicodeString titleString,UnicodeString bodyString,GameWinMsgBoxFunc yesCallback,GameWinMsgBoxFunc noCallback);  ///< convenience function for displaying a Message box with Yes and No buttons


GameWindow *MessageBoxYesNoCancel(UnicodeString titleString,UnicodeString bodyString, GameWinMsgBoxFunc yesCallback, GameWinMsgBoxFunc noCallback, GameWinMsgBoxFunc cancelCallback);///< convenience function for displaying a Message box with Yes,No and Cancel buttons


GameWindow *MessageBoxOkCancel(UnicodeString titleString,UnicodeString bodyString,GameWinMsgBoxFunc okCallback,GameWinMsgBoxFunc cancelCallback);///< convenience function for displaying a Message box with Ok and Cancel buttons

GameWindow *MessageBoxOk(UnicodeString titleString,UnicodeString bodyString,GameWinMsgBoxFunc okCallback);///< convenience function for displaying a Message box with Ok button


GameWindow *MessageBoxCancel(UnicodeString titleString,UnicodeString bodyString,GameWinMsgBoxFunc cancelCallback);///< convenience function for displaying a Message box with Cancel button

#endif //__MESSAGEBOX_H_
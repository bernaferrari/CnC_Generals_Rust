// FILE: WindowXlat.h ///////////////////////////////////////////////////////////
// Author: Steven Johnson, Dec 2001

#pragma once

#ifndef _H_WindowXlat
#define _H_WindowXlat

#include "GameClient/InGameUI.h"

//-----------------------------------------------------------------------------
class WindowTranslator : public GameMessageTranslator                          
{
private:
	// nothing
public:
	WindowTranslator();
	~WindowTranslator();
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};	

#endif

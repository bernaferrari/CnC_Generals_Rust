// FILE: HintSpy.h ///////////////////////////////////////////////////////////
// Author: Steven Johnson, Dec 2001

#pragma once

#ifndef _H_HintSpy
#define _H_HintSpy

#include "GameClient/InGameUI.h"

//-----------------------------------------------------------------------------
class HintSpyTranslator : public GameMessageTranslator
{
public:
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
	virtual ~HintSpyTranslator() { }
};	

#endif

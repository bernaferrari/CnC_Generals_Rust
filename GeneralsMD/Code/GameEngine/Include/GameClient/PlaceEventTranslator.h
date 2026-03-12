// FILE: PlaceEventTranslator.h ///////////////////////////////////////////////////////////
// Author: Steven Johnson, Dec 2001

#pragma once

#ifndef _H_PlaceEventTranslator
#define _H_PlaceEventTranslator

#include "GameClient/InGameUI.h"

//-----------------------------------------------------------------------------
class PlaceEventTranslator : public GameMessageTranslator                          
{
private:
	UnsignedInt m_frameOfUpButton;

public:
	PlaceEventTranslator();
	~PlaceEventTranslator();
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};	

#endif

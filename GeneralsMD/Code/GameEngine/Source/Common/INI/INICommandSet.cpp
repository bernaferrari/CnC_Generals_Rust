// FILE: INICommandSet.cpp ////////////////////////////////////////////////////////////////////////
// Author: Colin Day, March 2002
// Desc:   Command sets are a configurable set of CommandButtons, we will use the sets as
//				 part of the context sensitive user interface
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "GameClient/ControlBar.h"

//-------------------------------------------------------------------------------------------------
/** Parse a command set */
//-------------------------------------------------------------------------------------------------
void INI::parseCommandSetDefinition( INI *ini )
{
	ControlBar::parseCommandSetDefinition(ini);
}  // end parseCommandSetDefinition

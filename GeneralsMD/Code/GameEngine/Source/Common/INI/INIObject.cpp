// FILE: INIObject.cpp ////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, November 2001
// Desc:   Parsing Object INI entries
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "Common/ThingTemplate.h"
#include "Common/ThingFactory.h"
#include "GameLogic/Module/OpenContain.h"

///////////////////////////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
/** Parse Object entry */
//-------------------------------------------------------------------------------------------------
void INI::parseObjectDefinition( INI* ini )
{
	AsciiString name = ini->getNextToken();
	ThingFactory::parseObjectDefinition(ini, name, AsciiString::TheEmptyString);
}

//-------------------------------------------------------------------------------------------------
/** Parse Object entry */
//-------------------------------------------------------------------------------------------------
void INI::parseObjectReskinDefinition( INI* ini )
{
	AsciiString name = ini->getNextToken();
	AsciiString reskinFrom = ini->getNextToken();
	ThingFactory::parseObjectDefinition(ini, name, reskinFrom);
}



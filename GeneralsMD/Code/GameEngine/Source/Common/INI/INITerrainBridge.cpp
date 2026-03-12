// FILE: INITerrainBridge.cpp /////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   Terrain bridge INI loading
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "GameClient/TerrainRoads.h"

//-------------------------------------------------------------------------------------------------
/** Parse Terrain Bridge entry */
//-------------------------------------------------------------------------------------------------
void INI::parseTerrainBridgeDefinition( INI* ini )
{
	AsciiString name;
	TerrainRoadType *bridge;

	// read the name
	const char* c = ini->getNextToken();
	name.set( c );	

	// find existing item if present or allocate new one
	bridge = TheTerrainRoads->findBridge( name );

	// if item is found it better already be a bridge
	if( bridge )
	{

		// sanity
		DEBUG_ASSERTCRASH( bridge->isBridge(), ("Redefining road '%s' as a bridge!\n", 
											 bridge->getName().str()) );
		throw INI_INVALID_DATA;

	}  // end if

	if( bridge == NULL )	
		bridge = TheTerrainRoads->newBridge( name );

	DEBUG_ASSERTCRASH( bridge, ("Unable to allcoate bridge '%s'\n", name.str()) );

	// parse the ini definition
	ini->initFromINI( bridge, bridge->getBridgeFieldParse() );

}  // end parseTerrainBridge





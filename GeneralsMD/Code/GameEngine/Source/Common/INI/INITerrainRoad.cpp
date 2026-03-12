// FILE: INITerrainRoad.cpp ///////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   Terrain road INI loading
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "GameClient/TerrainRoads.h"

//-------------------------------------------------------------------------------------------------
/** Parse Terrain Road entry */
//-------------------------------------------------------------------------------------------------
void INI::parseTerrainRoadDefinition( INI* ini )
{
	AsciiString name;
	TerrainRoadType *road;

	// read the name
	const char* c = ini->getNextToken();
	name.set( c );	

	// find existing item if present or allocate new one
	road = TheTerrainRoads->findRoad( name );

	// if item is found it better not already be a bridge
	if( road )
	{

		// sanity
		DEBUG_ASSERTCRASH( road->isBridge() == FALSE, ("Redefining bridge '%s' as a road!\n", 
											 road->getName().str()) );
		throw INI_INVALID_DATA;

	}  // end if

	if( road == NULL )	
		road = TheTerrainRoads->newRoad( name );

	DEBUG_ASSERTCRASH( road, ("Unable to allocate road '%s'\n", name.str()) );

	// parse the ini definition
	ini->initFromINI( road, road->getRoadFieldParse() );

}  // end parseTerrainRoad





// FILE: INITerrain.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   Terrain type INI loading
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "Common/TerrainTypes.h"

//-------------------------------------------------------------------------------------------------
/** Parse Terrain type entry */
//-------------------------------------------------------------------------------------------------
void INI::parseTerrainDefinition( INI* ini )
{
	AsciiString name;
	TerrainType *terrainType;

	// read the name
	const char* c = ini->getNextToken();
	name.set( c );	

	// find existing item if present
	terrainType = TheTerrainTypes->findTerrain( name );
	if( terrainType == NULL )
		terrainType = TheTerrainTypes->newTerrain( name );

	// sanity
	DEBUG_ASSERTCRASH( terrainType, ("Unable to allocate terrain type '%s'\n", name.str()) );

	// parse the ini definition
	ini->initFromINI( terrainType, terrainType->getFieldParse() );

}  // end parseTerrain




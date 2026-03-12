// FILE: TerrainTypes.cpp /////////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   Terrain type descriptions and collection
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_TERRAIN_TYPE_NAMES

#include "Common/INI.h"
#include "Common/TerrainTypes.h"

// PUBLIC DATA ////////////////////////////////////////////////////////////////////////////////////
TerrainTypeCollection *TheTerrainTypes = NULL;

// PRIVATE DATA ///////////////////////////////////////////////////////////////////////////////////
const FieldParse TerrainType::m_terrainTypeFieldParseTable[] = 
{

	{ "Texture",		INI::parseAsciiString,			NULL,		offsetof( TerrainType, m_texture ) },
	{ "BlendEdges", INI::parseBool,							NULL,		offsetof( TerrainType, m_blendEdgeTexture ) },
	{ "Class",			INI::parseIndexList,				terrainTypeNames, offsetof( TerrainType, m_class ) },
	{ "RestrictConstruction", INI::parseBool,		NULL,		offsetof( TerrainType, m_restrictConstruction ) },

	{ NULL,					NULL,												NULL,		0 },

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
TerrainType::TerrainType( void )
{

	m_name.clear();
	m_texture.clear();
	m_blendEdgeTexture = FALSE;
	m_class = TERRAIN_NONE;
	m_restrictConstruction = FALSE;
	m_next = NULL;

}  // end TerrainType

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
TerrainType::~TerrainType( void )
{

}  // end ~TerrainType

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
TerrainTypeCollection::TerrainTypeCollection( void )
{

	m_terrainList = NULL;

}  // end TerrainTypeCollection

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
TerrainTypeCollection::~TerrainTypeCollection( void )
{
	TerrainType *temp;

	// delete all the type instances
	while( m_terrainList )
	{

		// get the next element
		temp = m_terrainList->friend_getNext();

		// delete the head of the type list
		m_terrainList->deleteInstance();

		// set the new head of the type list
		m_terrainList = temp;

	}  // end while

}  // end ~TerrainTypeCollection

//-------------------------------------------------------------------------------------------------
/** Find a terrain type given the name */
//-------------------------------------------------------------------------------------------------
TerrainType *TerrainTypeCollection::findTerrain( AsciiString name )
{
	TerrainType *terrain;

	for( terrain = m_terrainList; terrain; terrain = terrain->friend_getNext() )
	{

		if( terrain->getName() == name )
			return terrain;

	}  // end for terrain

	// not found
	return NULL;

}  // end findTerrain

//-------------------------------------------------------------------------------------------------
/** Allocate a new type, assign the name, and tie to type list */
//-------------------------------------------------------------------------------------------------
TerrainType *TerrainTypeCollection::newTerrain( AsciiString name )
{
	TerrainType *terrain = NULL;

	// allocate a new type
	terrain = newInstance(TerrainType);

	// copy default values from the default terrain entry
	TerrainType *defaultTerrain = findTerrain( AsciiString( "DefaultTerrain" ) );
	if( defaultTerrain )
		*terrain = *defaultTerrain;
/*
	{

		terrain->friend_setTexture( defaultTerrain->getTexture() );
		terrain->friend_setClass( defaultTerrain->getClass() );
		terrain->friend_setBlendEdge( defaultTerrain->isBlendEdge() );
			
	}  // end if
*/

	// assign a name
	terrain->friend_setName( name );

	// tie to list
	terrain->friend_setNext( m_terrainList );
	m_terrainList = terrain;
			
	// return the new terrain
	return terrain;

}  // end newTerrain

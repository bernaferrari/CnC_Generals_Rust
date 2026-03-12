// FILE: StructureBody.cpp ////////////////////////////////////////////////////////////////////////
// Author: Colin Day, November 2001
// Desc:	 Structure bodies are active bodies specifically for structures that are built
//				 and/or interactable (is that a world) with the player.
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Damage.h"
#include "GameLogic/Module/StructureBody.h"

// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
StructureBody::StructureBody( Thing *thing, const ModuleData* moduleData ) 
							: ActiveBody( thing, moduleData )
{

	m_constructorObjectID = INVALID_ID;

}  // end StructureBody

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
StructureBody::~StructureBody( void )
{

}  // end ~StructureBody

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void StructureBody::setConstructorObject( Object *obj )
{ 

	if( obj ) 
		m_constructorObjectID = obj->getID(); 

}  // end setConstructorObject

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void StructureBody::crc( Xfer *xfer )
{

	// extend base class
	ActiveBody::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void StructureBody::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// base class
	ActiveBody::xfer( xfer );

	// constructor object id
	xfer->xferObjectID( &m_constructorObjectID );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void StructureBody::loadPostProcess( void )
{

	// extend base class
	ActiveBody::loadPostProcess();

}  // end loadPostProcess

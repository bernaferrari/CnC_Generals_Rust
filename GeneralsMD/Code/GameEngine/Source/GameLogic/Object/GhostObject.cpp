// FILE GhostObject.cpp ///////////////////////////////////////////////////////////////////////////
// Simple base object
// Author: Michael S. Booth, October 2000
///////////////////////////////////////////////////////////////////////////////////////////////////
 
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/GhostObject.h"
#include "GameLogic/Object.h"

GhostObjectManager *TheGhostObjectManager=NULL;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
GhostObject::GhostObject(void):
//Added By Sadullah Nader
//Initializations missing and needed
m_parentAngle(0.0f),
m_parentGeometryIsSmall(0.0f),
m_parentGeometryMajorRadius(0.0f),
m_parentGeometryminorRadius(0.0f),
m_parentObject(NULL),
m_partitionData(NULL)
{ 
	m_parentPosition.zero();
	// End Initializations
}  // end Object

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
GhostObject::~GhostObject()
{

}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void GhostObject::crc( Xfer *xfer )
{

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer Method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void GhostObject::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// parent object
	ObjectID parentObjectID = INVALID_ID;
	if( m_parentObject )
		parentObjectID = m_parentObject->getID();
	xfer->xferObjectID( &parentObjectID );
	if( xfer->getXferMode() == XFER_LOAD )
	{

		// tie up parent object pointer
		m_parentObject = TheGameLogic->findObjectByID( parentObjectID );

		// sanity
		if( parentObjectID != INVALID_ID && m_parentObject == NULL )
		{

			DEBUG_CRASH(( "GhostObject::xfer - Unable to connect m_parentObject\n" ));
			throw INI_INVALID_DATA;

		}  // end if

	}  // end if

	// parent geometry type
	xfer->xferUser( &m_parentGeometryType, sizeof( GeometryType ) );

	// parent geometry is small
	xfer->xferBool( &m_parentGeometryIsSmall );

	// parent geometry major radius
	xfer->xferReal( &m_parentGeometryMajorRadius );

	// parent geometry minor radius
	xfer->xferReal( &m_parentGeometryminorRadius );

	// parent angle
	xfer->xferReal( &m_parentAngle );

	// parent position
	xfer->xferCoord3D( &m_parentPosition );

	// partition data
	///@todo write me ---> !!!!!
	// PartitionData	*m_partitionData;	///< our PartitionData

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void GhostObject::loadPostProcess( void )
{

}  // end loadPostProcess

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
GhostObjectManager::GhostObjectManager(void)
{
	m_lockGhostObjects = FALSE;
	m_saveLockGhostObjects = FALSE;
	m_localPlayer = 0;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
GhostObjectManager::~GhostObjectManager()
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::reset(void)
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
GhostObject *GhostObjectManager::addGhostObject(Object *object, PartitionData *pd)
{
	return 0;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::removeGhostObject(GhostObject *mod)
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::updateOrphanedObjects(int *playerIndexList, int numNonLocalPlayers)
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::releasePartitionData(void)
{
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::restorePartitionData(void)
{
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::crc( Xfer *xfer )
{

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer Method:
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// local player
	xfer->xferInt( &m_localPlayer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void GhostObjectManager::loadPostProcess( void )
{

}  // end loadPostProcess

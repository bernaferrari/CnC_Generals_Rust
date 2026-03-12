// FILE: StealthUpgrade.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, May 2002
// Desc:	 An upgrade that set's the owner's OBJECT_STATUS_CAN_STEALTH Status
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameLogic/Module/StealthUpgrade.h"
#include "GameLogic/Module/SpawnBehavior.h"
#include "GameLogic/Object.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
StealthUpgrade::StealthUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
StealthUpgrade::~StealthUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
void StealthUpgrade::upgradeImplementation( )
{
	// The logic that does the stealthupdate will notice this and start stealthing
	Object *me = getObject();
	me->setStatus( MAKE_OBJECT_STATUS_MASK( OBJECT_STATUS_CAN_STEALTH ) );
	
	//Grant stealth to spawns if applicable.
	if( me->isKindOf( KINDOF_SPAWNS_ARE_THE_WEAPONS ) )
	{
		SpawnBehaviorInterface *sbInterface = me->getSpawnBehaviorInterface();
		if( sbInterface )
		{
			sbInterface->giveSlavesStealthUpgrade( TRUE );
		}
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void StealthUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void StealthUpgrade::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpgradeModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void StealthUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess

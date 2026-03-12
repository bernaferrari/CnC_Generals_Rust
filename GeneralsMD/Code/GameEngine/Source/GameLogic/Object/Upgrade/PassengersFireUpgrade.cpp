// FILE: PassengersFireUpgrade.h /////////////////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, May 2003
// Desc:	 UpgradeModule that sets containmodules flag for passengersAllowedToFire
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/PassengersFireUpgrade.h"
#include "GameLogic/Module/ContainModule.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PassengersFireUpgrade::PassengersFireUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PassengersFireUpgrade::~PassengersFireUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void PassengersFireUpgrade::upgradeImplementation( )
{
	// Just need to flag the containmodule having the passengersallowedtofire true, .

  
  Object *obj = getObject();
//	obj->setWeaponSetFlag( WEAPONSET_PLAYER_UPGRADE );

  ContainModuleInterface *contain = obj->getContain();  
  if ( contain )
  {
    contain->setPassengerAllowedToFire( TRUE );
  }

}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void PassengersFireUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void PassengersFireUpgrade::xfer( Xfer *xfer )
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
void PassengersFireUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess

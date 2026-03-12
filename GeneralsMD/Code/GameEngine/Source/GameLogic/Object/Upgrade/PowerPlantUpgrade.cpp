// FILE: PowerPlantUpgrade.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Amit Kumar, August 2002
// Desc:	 Power plant upgrades
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/ModelState.h"
#include "Common/Player.h"
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameClient/InGameUI.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/PowerPlantUpdate.h"
#include "GameLogic/Module/PowerPlantUpgrade.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PowerPlantUpgrade::PowerPlantUpgrade( Thing *thing, const ModuleData* moduleData ) : 
							UpgradeModule( thing, moduleData )
{

}  // end PowerPlantUpgrade

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PowerPlantUpgrade::~PowerPlantUpgrade( void )
{

}  // end ~PowerPlantUpgrade

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void PowerPlantUpgrade::onDelete( void )
{

	// if we haven't been upgraded there is nothing to clean up
	if( isAlreadyUpgraded() == FALSE )
		return;

	// remove the power bonus from the player
	Player *player = getObject()->getControllingPlayer();
	if( player )
		player->removePowerBonus( getObject() );

	// this upgrade module is now "not upgraded"
	setUpgradeExecuted(FALSE);

}  // end onDelete

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void PowerPlantUpgrade::onCapture( Player *oldOwner, Player *newOwner )
{

	// do nothing if we haven't upgraded yet
	if( isAlreadyUpgraded() == FALSE )
		return;
	
	if (getObject()->isDisabled())
		return;

	// remove power bonus from old owner
	if( oldOwner )
	{

		oldOwner->removePowerBonus( getObject() );
		setUpgradeExecuted(FALSE);

	}

	// add power bonus to the new owner
	if( newOwner )
	{

		newOwner->addPowerBonus( getObject() );
		setUpgradeExecuted(TRUE);

	}

}  // end onCapture

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void PowerPlantUpgrade::upgradeImplementation( void )
{

	Player *player = getObject()->getControllingPlayer();
	
	// add the new power production to the object
	if( player )
		player->addPowerBonus(getObject());


	PowerPlantUpdateInterface *ppui;
	for( BehaviorModule **umi = getObject()->getBehaviorModules(); *umi; ++umi)
	{
		ppui = (*umi)->getPowerPlantUpdateInterface();
		if( ppui )
			ppui->extendRods(TRUE);
	}
	
}  // end upgradeImplementation

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void PowerPlantUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void PowerPlantUpgrade::xfer( Xfer *xfer )
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
void PowerPlantUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();
	
	// Most upgrade modules have state change effects that are themselves saved.  This one is a fire and forget.
	// So we need to re-fire on load if we are turned on.
	if( isAlreadyUpgraded() )
	{
		Player *player = getObject()->getControllingPlayer();
		
		// add the new power production to the object
		if( player )
			player->addPowerBonus(getObject());
	}

}  // end loadPostProcess

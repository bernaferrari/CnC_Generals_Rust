// FILE: UpgradeDie.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, August 2002
// Desc:  When object dies, the parent object is freed of the specified object upgrade field.
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/ThingTemplate.h"
#include "Common/Upgrade.h"
#include "Common/Xfer.h"

#include "GameClient/Drawable.h"

#include "GameLogic/GameLogic.h"
#include "GameLogic/Module/UpgradeDie.h"
#include "GameLogic/Object.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpgradeDie::UpgradeDie( Thing *thing, const ModuleData* moduleData ) : DieModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpgradeDie::~UpgradeDie( void )
{
}

//-------------------------------------------------------------------------------------------------
/** The die callback. */
//-------------------------------------------------------------------------------------------------
void UpgradeDie::onDie( const DamageInfo *damageInfo )
{
	if (!isDieApplicable(damageInfo))
		return;
	//Look for the object that created me.
	Object *producer = TheGameLogic->findObjectByID( getObject()->getProducerID() );
	if( producer )
	{
		//Okay, we found our parent... now look for the upgrade.
		const UpgradeTemplate *upgrade = TheUpgradeCenter->findUpgrade( getUpgradeDieModuleData()->m_upgradeName );

		if( upgrade )
		{
			//We found the upgrade, now see if the parent object has it set...
			if( producer->hasUpgrade( upgrade ) )
			{
				//Cool, now remove it.
				producer->removeUpgrade( upgrade );
			}
			else
			{
				DEBUG_ASSERTCRASH( 0, ("Object %s just died, but is trying to free upgrade %s in it's producer %s%s",
					getObject()->getTemplate()->getName().str(),
					getUpgradeDieModuleData()->m_upgradeName.str(),
					producer->getTemplate()->getName().str(), 
					", which the producer doesn't have. This is used in cases where the producer builds an upgrade that can die... like ranger building scout drones.") );
			}
		}
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void UpgradeDie::crc( Xfer *xfer )
{

	// extend base class
	DieModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void UpgradeDie::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	DieModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void UpgradeDie::loadPostProcess( void )
{

	// extend base class
	DieModule::loadPostProcess();

}  // end loadPostProcess

// FILE: FXListDie.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, Jan 2002
// Desc:   Simple Die module
///////////////////////////////////////////////////////////////////////////////////////////////////


// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "Common/Player.h"
#include "Common/ThingTemplate.h"
#include "Common/Xfer.h"
#include "GameClient/FXList.h"
#include "GameLogic/Damage.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/FXListDie.h"
#include "GameLogic/Module/AIUpdate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
FXListDie::FXListDie( Thing *thing, const ModuleData* moduleData ) : DieModule( thing, moduleData )
{
	if( getFXListDieModuleData()->m_initiallyActive )
	{
		giveSelfUpgrade();
	}
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
FXListDie::~FXListDie( void )
{

}

//-------------------------------------------------------------------------------------------------
/** The die callback. */
//-------------------------------------------------------------------------------------------------
void FXListDie::onDie( const DamageInfo *damageInfo )
{
	if (!isUpgradeActive())
		return;
	if (!isDieApplicable(damageInfo))
		return;
	const FXListDieModuleData* d = getFXListDieModuleData();

	UpgradeMaskType activation, conflicting;
	getUpgradeActivationMasks( activation, conflicting );
	Object *obj = getObject();
	if( obj->getObjectCompletedUpgradeMask().testForAny( conflicting ) )
	{
		return;
	}
	if( obj->getControllingPlayer() && obj->getControllingPlayer()->getCompletedUpgradeMask().testForAny( conflicting ) )
	{
		return;
	}

	if (d->m_defaultDeathFX)
	{
		if (d->m_orientToObject)
		{
			Object *damageDealer = TheGameLogic->findObjectByID( damageInfo->in.m_sourceID );
			FXList::doFXObj(getFXListDieModuleData()->m_defaultDeathFX, getObject(), damageDealer);
		}
		else
		{
			FXList::doFXPos(getFXListDieModuleData()->m_defaultDeathFX, getObject()->getPosition());
		}
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void FXListDie::crc( Xfer *xfer )
{

	// extend base class
	DieModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void FXListDie::xfer( Xfer *xfer )
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
void FXListDie::loadPostProcess( void )
{

	// extend base class
	DieModule::loadPostProcess();

}  // end loadPostProcess

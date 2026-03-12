// FILE: FireWeaponWhenDeadBehavior.cpp ///////////////////////////////////////////////////////////////////////
// Author:
// Desc:  
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#define DEFINE_SLOWDEATHPHASE_NAMES

#include "Common/Thing.h"
#include "Common/ThingTemplate.h"
#include "Common/INI.h"
#include "Common/RandomValue.h"
#include "Common/Xfer.h"
#include "Common/Player.h"
#include "GameClient/Drawable.h"
#include "GameClient/FXList.h"
#include "GameClient/InGameUI.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Module/BodyModule.h"
#include "GameLogic/Module/FireWeaponWhenDeadBehavior.h"
#include "GameLogic/Module/AIUpdate.h"
#include "GameLogic/Object.h"
#include "GameLogic/ObjectCreationList.h"
#include "GameLogic/Weapon.h"
#include "GameClient/Drawable.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

const Int MAX_IDX = 32;

const Real BEGIN_MIDPOINT_RATIO = 0.35f;
const Real END_MIDPOINT_RATIO = 0.65f;


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
FireWeaponWhenDeadBehavior::FireWeaponWhenDeadBehavior( Thing *thing, const ModuleData* moduleData ) : 
	BehaviorModule( thing, moduleData )
{
	if (getFireWeaponWhenDeadBehaviorModuleData()->m_initiallyActive)
	{
		giveSelfUpgrade();
	}
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
FireWeaponWhenDeadBehavior::~FireWeaponWhenDeadBehavior( void )
{
}

//-------------------------------------------------------------------------------------------------
/** The die callback. */
//-------------------------------------------------------------------------------------------------
void FireWeaponWhenDeadBehavior::onDie( const DamageInfo *damageInfo )
{
	const FireWeaponWhenDeadBehaviorModuleData* d = getFireWeaponWhenDeadBehaviorModuleData();
	Object *obj = getObject();

	if (!isUpgradeActive())
		return;

	// right type?
	if (!d->m_dieMuxData.isDieApplicable(getObject(), damageInfo))
		return;
	
	// This will never apply until built.  Otherwise canceling construction sets it off, and killing
	// a one hitpoint one percent building will too.
	if( obj->getStatusBits().test( OBJECT_STATUS_UNDER_CONSTRUCTION ) )
		return;

	
	UpgradeMaskType activation, conflicting;
	getUpgradeActivationMasks( activation, conflicting );
	
	if( obj->getObjectCompletedUpgradeMask().testForAny( conflicting ) )
	{
		return;
	}
	if( obj->getControllingPlayer() && obj->getControllingPlayer()->getCompletedUpgradeMask().testForAny( conflicting ) )
	{
		return;
	}
	
	if (d->m_deathWeapon)
	{
		// fire the default weapon
	  TheWeaponStore->createAndFireTempWeapon(d->m_deathWeapon, obj, obj->getPosition());
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void FireWeaponWhenDeadBehavior::crc( Xfer *xfer )
{

	// extend base class
	BehaviorModule::crc( xfer );

	// extend upgrade mux
	UpgradeMux::upgradeMuxCRC( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void FireWeaponWhenDeadBehavior::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	BehaviorModule::xfer( xfer );

	// extend upgrade mux
	UpgradeMux::upgradeMuxXfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void FireWeaponWhenDeadBehavior::loadPostProcess( void )
{

	// extend base class
	BehaviorModule::loadPostProcess();

	// extend upgrade mux
	UpgradeMux::upgradeMuxLoadPostProcess();

}  // end loadPostProcess

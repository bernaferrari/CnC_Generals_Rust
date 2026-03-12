// FILE: SubdualDamageHelper.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:   Object helper - Clears subdual status and heals subdual damage since Body modules can't have Updates
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"

#include "GameLogic/Object.h"
#include "GameLogic/Module/BodyModule.h"
#include "GameLogic/Module/SubdualDamageHelper.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
SubdualDamageHelper::SubdualDamageHelper( Thing *thing, const ModuleData *modData ) : ObjectHelper( thing, modData ) 
{ 
	m_healingStepCountdown = 0;

	setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
SubdualDamageHelper::~SubdualDamageHelper( void )
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
UpdateSleepTime SubdualDamageHelper::update()
{
	BodyModuleInterface *body = getObject()->getBodyModule();

	m_healingStepCountdown--;
	if( m_healingStepCountdown > 0 )
		return UPDATE_SLEEP_NONE;

	m_healingStepCountdown = body->getSubdualDamageHealRate();

	DamageInfo removeSubdueDamage;
	removeSubdueDamage.in.m_damageType = DAMAGE_SUBDUAL_UNRESISTABLE;
	removeSubdueDamage.in.m_amount = -body->getSubdualDamageHealAmount();
	body->attemptDamage(&removeSubdueDamage);

	if( body->hasAnySubdualDamage() )
		return UPDATE_SLEEP_NONE; 
	else
		return UPDATE_SLEEP_FOREVER;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void SubdualDamageHelper::notifySubdualDamage( Real amount )
{
	if( amount > 0 )
	{
		m_healingStepCountdown = getObject()->getBodyModule()->getSubdualDamageHealRate();
		setWakeFrame(getObject(), UPDATE_SLEEP_NONE);
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void SubdualDamageHelper::crc( Xfer *xfer )
{

	// object helper crc
	ObjectHelper::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info;
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void SubdualDamageHelper::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// object helper base class
	ObjectHelper::xfer( xfer );

	xfer->xferUnsignedInt( &m_healingStepCountdown );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void SubdualDamageHelper::loadPostProcess( void )
{

	// object helper base class
	ObjectHelper::loadPostProcess();

}  // end loadPostProcess


// FILE: TempWeaponBonusHelper.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:   Object helper - Clears Temporary weapon bonus effects
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"

#include "GameLogic/Module/TempWeaponBonusHelper.h"

#include "GameClient/Drawable.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Weapon.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
TempWeaponBonusHelper::TempWeaponBonusHelper( Thing *thing, const ModuleData *modData ) : ObjectHelper( thing, modData ) 
{ 
	m_currentBonus = WEAPONBONUSCONDITION_INVALID;
	m_frameToRemove = 0;

	setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
TempWeaponBonusHelper::~TempWeaponBonusHelper( void )
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
UpdateSleepTime TempWeaponBonusHelper::update()
{
	DEBUG_ASSERTCRASH(m_frameToRemove <= TheGameLogic->getFrame(), ("TempWeaponBonusHelper woke up too soon.") );

	clearTempWeaponBonus(); // We are sleep driven, so seeing an update means our timer is ready implicitly
	return UPDATE_SLEEP_FOREVER;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void TempWeaponBonusHelper::clearTempWeaponBonus()
{
	if( m_currentBonus != WEAPONBONUSCONDITION_INVALID )
	{
		getObject()->clearWeaponBonusCondition(m_currentBonus);
		m_currentBonus = WEAPONBONUSCONDITION_INVALID;
		m_frameToRemove = 0;

		if( getObject()->getDrawable() )
    {
			getObject()->getDrawable()->clearTintStatus(TINT_STATUS_FRENZY);
//      if (getObject()->isKindOf(KINDOF_INFANTRY))
//        getObject()->getDrawable()->setSecondMaterialPassOpacity( 0.0f );
    }
	}
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void TempWeaponBonusHelper::doTempWeaponBonus( WeaponBonusConditionType status, UnsignedInt duration )
{
	// Clear any different status we may have.  Re-getting the same status will just reset the timer
	if( m_currentBonus != status )
		clearTempWeaponBonus();

	getObject()->setWeaponBonusCondition(status);
	m_currentBonus = status;
	m_frameToRemove = TheGameLogic->getFrame() + duration;

	if( getObject()->getDrawable() )
  {
		getObject()->getDrawable()->setTintStatus(TINT_STATUS_FRENZY);
//    if (getObject()->isKindOf(KINDOF_INFANTRY))
//      getObject()->getDrawable()->setSecondMaterialPassOpacity( 1.0f );
  }

	setWakeFrame( getObject(), UPDATE_SLEEP(duration) );
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void TempWeaponBonusHelper::crc( Xfer *xfer )
{

	// object helper crc
	ObjectHelper::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info;
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void TempWeaponBonusHelper::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// object helper base class
	ObjectHelper::xfer( xfer );

	xfer->xferUser( &m_currentBonus, sizeof(WeaponBonusConditionType) );// an enum
	xfer->xferUnsignedInt( &m_frameToRemove );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void TempWeaponBonusHelper::loadPostProcess( void )
{

	// object helper base class
	ObjectHelper::loadPostProcess();

}  // end loadPostProcess


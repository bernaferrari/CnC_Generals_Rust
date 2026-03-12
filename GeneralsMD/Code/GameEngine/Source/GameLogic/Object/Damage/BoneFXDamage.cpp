// FILE: BoneFXDamage.cpp ///////////////////////////////////////////////////////////////////
// Author: Bryan Cleveland, March 2002
// Desc:   Damage module that goes with BoneFX update
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/BoneFXDamage.h"
#include "GameLogic/Module/BoneFXUpdate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
BoneFXDamage::BoneFXDamage( Thing *thing, const ModuleData* moduleData ) 
																		  : DamageModule( thing, moduleData )
{
}  // end BoneFXDamage

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
BoneFXDamage::~BoneFXDamage( void )
{

}  // end ~BoneFXDamage

//-------------------------------------------------------------------------------------------------
void BoneFXDamage::onObjectCreated()
{
	static NameKeyType key_BoneFXUpdate = NAMEKEY("BoneFXUpdate");
	BoneFXUpdate* bfxu = (BoneFXUpdate*)getObject()->findUpdateModule(key_BoneFXUpdate);
	if (bfxu == NULL)
	{
		DEBUG_ASSERTCRASH(bfxu != NULL, ("BoneFXDamage requires BoneFXUpdate"));
		throw INI_INVALID_DATA;
	}
}

//-------------------------------------------------------------------------------------------------
/** Switching damage states */
//-------------------------------------------------------------------------------------------------
void BoneFXDamage::onBodyDamageStateChange( const DamageInfo *damageInfo, 
																						BodyDamageType oldState, 
																						BodyDamageType newState )
{
	static NameKeyType key_BoneFXUpdate = NAMEKEY("BoneFXUpdate");
	BoneFXUpdate* bfxu = (BoneFXUpdate*)getObject()->findUpdateModule(key_BoneFXUpdate);
	if (bfxu)
		bfxu->changeBodyDamageState(oldState, newState);

}  // end onBodyDamageStateChange

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void BoneFXDamage::crc( Xfer *xfer )
{

	// extend base class
	DamageModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void BoneFXDamage::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	DamageModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void BoneFXDamage::loadPostProcess( void )
{

	// extend base class
	DamageModule::loadPostProcess();

}  // end loadPostProcess

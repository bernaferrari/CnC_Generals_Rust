// FILE: FireWeaponPower.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	Created:	August 2003
//
//	Filename: FireWeaponPower.cpp
//
//	Author:		Kris Morness
//
//  Purpose:	Simply loads and fires a specific weapon controlled by a superweapon timer.
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_WEAPONSLOTTYPE_NAMES //For access to TheWeaponSlotTypeNames

#include "Common/Player.h"
#include "Common/ThingFactory.h"
#include "Common/Xfer.h"

#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/WeaponSet.h"

#include "GameLogic/Module/AIUpdate.h"
#include "GameLogic/Module/FireWeaponPower.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

FireWeaponPowerModuleData::FireWeaponPowerModuleData( void )
{
	m_maxShotsToFire = 1;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
/*static*/ void FireWeaponPowerModuleData::buildFieldParse(MultiIniFieldParse& p)
{
	SpecialPowerModuleData::buildFieldParse( p );
	
	static const FieldParse dataFieldParse[] = 
	{
		{ "MaxShotsToFire", INI::parseUnsignedInt, NULL, offsetof( FireWeaponPowerModuleData, m_maxShotsToFire ) },
		{ 0, 0, 0, 0 }
	};
	p.add(dataFieldParse);
	
}  // end buildFieldParse


// ------------------------------------------------------------------------------------------------
FireWeaponPower::FireWeaponPower( Thing *thing, const ModuleData *moduleData )
												: SpecialPowerModule( thing, moduleData )
{

}

// ------------------------------------------------------------------------------------------------
FireWeaponPower::~FireWeaponPower( void )
{

} 

// ------------------------------------------------------------------------------------------------
void FireWeaponPower::doSpecialPower( UnsignedInt commandOptions )
{
	Object *self = getObject();
	const FireWeaponPowerModuleData *data = getFireWeaponPowerModuleData();

	if( self->isDisabled() )
		return;

	// call the base class action cause we are *EXTENDING* functionality
	SpecialPowerModule::doSpecialPower( commandOptions );

	self->reloadAllAmmo( TRUE );

	AIUpdateInterface *ai = self->getAI();
	if( ai )
	{
		ai->aiAttackPosition( NULL, data->m_maxShotsToFire, CMD_FROM_AI );

		//Order any turrets to attack as well.
		for( Int i = 0; i < MAX_TURRETS; i++ )
		{
			ai->setTurretTargetPosition( (WhichTurretType)i, self->getPosition() );
		}
	}
}  

// ------------------------------------------------------------------------------------------------
void FireWeaponPower::doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions )
{
	Object *self = getObject();
	const FireWeaponPowerModuleData *data = getFireWeaponPowerModuleData();

	if( self->isDisabled() )
		return;

	// call the base class action cause we are *EXTENDING* functionality
	SpecialPowerModule::doSpecialPowerAtLocation( loc, angle, commandOptions );

	self->reloadAllAmmo( TRUE );

	AIUpdateInterface *ai = self->getAI();
	if( ai )
	{
		ai->aiAttackPosition( loc, data->m_maxShotsToFire, CMD_FROM_AI );

		//Order any turrets to attack as well.
		for( Int i = 0; i < MAX_TURRETS; i++ )
		{
			ai->setTurretTargetPosition( (WhichTurretType)i, loc );
		}
	}

}  

// ------------------------------------------------------------------------------------------------
void FireWeaponPower::doSpecialPowerAtObject( Object *obj, UnsignedInt commandOptions )
{
	Object *self = getObject();
	const FireWeaponPowerModuleData *data = getFireWeaponPowerModuleData();

	if( self->isDisabled() )
		return;

	// call the base class action cause we are *EXTENDING* functionality
	SpecialPowerModule::doSpecialPowerAtObject( obj, commandOptions );

	self->reloadAllAmmo( TRUE );

	AIUpdateInterface *ai = self->getAI();
	if( ai )
	{
		ai->aiAttackObject( obj, data->m_maxShotsToFire, CMD_FROM_AI );

		//Order any turrets to attack as well.
		for( Int i = 0; i < MAX_TURRETS; i++ )
		{
			ai->setTurretTargetObject( (WhichTurretType)i, obj );
		}
	}

}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void FireWeaponPower::crc( Xfer *xfer )
{

	// extend base class
	SpecialPowerModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void FireWeaponPower::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	SpecialPowerModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void FireWeaponPower::loadPostProcess( void )
{

	// extend base class
	SpecialPowerModule::loadPostProcess();

}  // end loadPostProcess

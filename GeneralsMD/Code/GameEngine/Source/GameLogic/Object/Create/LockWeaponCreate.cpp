// FILE: LockWeaponCreate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2003
// Desc:   Locks the weapon choice to the slot specified on creation
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_WEAPONSLOTTYPE_NAMES
#include "Common/Xfer.h"
#include "GameLogic/Module/LockWeaponCreate.h"
#include "GameLogic/Object.h"
#include "GameLogic/WeaponSet.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
LockWeaponCreateModuleData::LockWeaponCreateModuleData()
{
	m_slotToLock = PRIMARY_WEAPON;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void LockWeaponCreateModuleData::buildFieldParse(MultiIniFieldParse& p)
{
  CreateModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "SlotToLock",	INI::parseLookupList,	TheWeaponSlotTypeNamesLookupList, offsetof( LockWeaponCreateModuleData, m_slotToLock ) },
		{ 0, 0, 0, 0 }
	};

  p.add(dataFieldParse);
}

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
LockWeaponCreate::LockWeaponCreate( Thing *thing, const ModuleData* moduleData ) : CreateModule( thing, moduleData )
{
}  // end GrantUpgradeCreate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
LockWeaponCreate::~LockWeaponCreate( void )
{

}  // end ~GrantUpgradeCreate

//-------------------------------------------------------------------------------------------------
/** The create callback. */
//-------------------------------------------------------------------------------------------------
void LockWeaponCreate::onCreate( void )
{
}  // end onCreate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void LockWeaponCreate::onBuildComplete( void )
{
	CreateModule::onBuildComplete(); // extend

	Object *me = getObject();
	WeaponSlotType slot = getLockWeaponCreateModuleData()->m_slotToLock;
	me->setWeaponLock( slot, LOCKED_PERMANENTLY );
}  // end onBuildComplete

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void LockWeaponCreate::crc( Xfer *xfer )
{

	// extend base class
	CreateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void LockWeaponCreate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	CreateModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void LockWeaponCreate::loadPostProcess( void )
{

	// extend base class
	CreateModule::loadPostProcess();

}  // end loadPostProcess

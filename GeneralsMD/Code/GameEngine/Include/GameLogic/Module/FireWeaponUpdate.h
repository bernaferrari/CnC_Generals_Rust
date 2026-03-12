// FILE: FireWeaponUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, August 2002
// Desc:   Update fires a weapon at its own feet as quickly as the weapon allows
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FIRE_WEAPON_UPDATE_H_
#define __FIRE_WEAPON_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"
#include "GameLogic/Weapon.h"

//-------------------------------------------------------------------------------------------------
class FireWeaponUpdateModuleData : public UpdateModuleData
{
public:
	const WeaponTemplate* m_weaponTemplate;
  UnsignedInt m_initialDelayFrames;
	UnsignedInt m_exclusiveWeaponDelay;	///< If non-zero, any other weapon having fired this recently will keep us from doing anything
	
	FireWeaponUpdateModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);

private:

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class FireWeaponUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( FireWeaponUpdate, "FireWeaponUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( FireWeaponUpdate, FireWeaponUpdateModuleData )

public:

	FireWeaponUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();

protected:

	Bool isOkayToFire();
	
	Weapon* m_weapon;
  UnsignedInt m_initialDelayFrame;

};

#endif


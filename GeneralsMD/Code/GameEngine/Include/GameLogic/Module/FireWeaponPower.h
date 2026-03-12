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
//	Filename: FireWeaponPower.h
//
//	Author:		Kris Morness
//
//  Purpose:	Simply loads and fires a specific weapon controlled by a superweapon timer.
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FIRE_WEAPON_POWER_H
#define __FIRE_WEAPON_POWER_H

#include "GameLogic/Module/SpecialPowerModule.h"

class Object;
class SpecialPowerTemplate;
struct FieldParse;
enum ScienceType;

class FireWeaponPowerModuleData : public SpecialPowerModuleData
{

public:

	FireWeaponPowerModuleData( void );

	static void buildFieldParse( MultiIniFieldParse& p );

	UnsignedInt m_maxShotsToFire;
};


//-------------------------------------------------------------------------------------------------
class FireWeaponPower : public SpecialPowerModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( FireWeaponPower, "FireWeaponPower" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( FireWeaponPower, FireWeaponPowerModuleData )

public:

	FireWeaponPower( Thing *thing, const ModuleData *moduleData );

	virtual void doSpecialPower( UnsignedInt commandOptions );
	virtual void doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions );
	virtual void doSpecialPowerAtObject( Object *obj, UnsignedInt commandOptions );

protected:

};

#endif // __FIRE_WEAPON_POWER_H

// FILE: Special Ability.h ///////////////////////////////////////////////////////////////
// Author: Kris Morness, July 2002
// Desc:   This is the class that handles processing of any special attack from a unit. There are 
//         many different styles and rules for various attacks.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SPECIAL_ABILITY_H_
#define __SPECIAL_ABILITY_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/SpecialPowerModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class SpecialAbilityModuleData : public SpecialPowerModuleData
{
	// nothing
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class SpecialAbility : public SpecialPowerModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SpecialAbility, "SpecialAbility" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SpecialAbility, SpecialAbilityModuleData )

public:

	SpecialAbility( Thing *thing, const ModuleData *moduleData );

	virtual void doSpecialPowerAtObject( Object *obj, UnsignedInt commandOptions );
	virtual void doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions );
	virtual void doSpecialPower( UnsignedInt commandOptions );

protected:

};

#endif  // end __SPECIAL_ABILITY_H_
// FILE: HealCrateCollide.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that heals everything owned by the collider
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef HEAL_CRATE_COLLIDE_H_
#define HEAL_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class HealCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( HealCrateCollide, "HealCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO( HealCrateCollide );

public:

	HealCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );
};

#endif

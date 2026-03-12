// FILE: ShroudCrateCollide.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that clears the shroud for the pickerupper
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef SHROUD_CRATE_COLLIDE_H_
#define SHROUD_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class ShroudCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ShroudCrateCollide, "ShroudCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO( ShroudCrateCollide );

public:

	ShroudCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );
};

#endif
